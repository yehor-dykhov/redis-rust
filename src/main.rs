mod storage;

use crate::storage::{add as storage_add, get as storage_get, CommandData};
use std::borrow::ToOwned;
use std::cmp::PartialEq;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisCommandError {
    #[error("invalid Redis command")]
    Invalid(String),
    #[error("unknown command")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum RedisResponseCommandError {
    #[error("invalid response command")]
    Invalid(String),
    #[error("unknown command")]
    Unknown,
}

#[derive(PartialEq, Debug)]
enum RedisCommand {
    Ping,
    Echo,
    Set,
    Get,
}

impl FromStr for RedisCommand {
    type Err = RedisCommandError;

    fn from_str(s: &str) -> Result<RedisCommand, Self::Err> {
        match s.to_lowercase().as_str() {
            "ping" => Ok(RedisCommand::Ping),
            "echo" => Ok(RedisCommand::Echo),
            "set" => Ok(RedisCommand::Set),
            "get" => Ok(RedisCommand::Get),
            _ => Err(RedisCommandError::Invalid(s.to_string())),
        }
    }
}

#[derive(Debug)]
struct RedisCommandValue {
    command: RedisCommand,
    param_1: Option<String>,
    param_2: Option<String>,
    expires_for: Option<Duration>,
}

impl RedisCommandValue {
    fn new(
        command: RedisCommand,
        param_1: Option<String>,
        param_2: Option<String>,
        expires_for: Option<Duration>,
    ) -> Self {
        Self {
            command,
            param_1,
            param_2,
            expires_for,
        }
    }

    fn to_response(&self) -> String {
        match self.command {
            RedisCommand::Ping => "+PONG\r\n".to_owned(),
            RedisCommand::Set => "+OK\r\n".to_owned(),
            RedisCommand::Get => {
                if let Some(cd) = storage_get(self.param_2.clone().unwrap().as_str()) {
                    println!("CD: {:?}", &cd.clone());
                    if filter_expired(&cd).is_some() {
                        let len = cd.value.len();
                        return format!("${len}\r\n{}\r\n", cd.value);
                    }
                }

                "$-1\r\n".to_string()
            }
            RedisCommand::Echo => {
                let len = self.param_2.clone().unwrap_or("".to_string()).len();
                format!("${len}\r\n{}\r\n", self.param_2.as_ref().unwrap())
            }
        }
    }
}

fn filter_expired(data: &CommandData) -> Option<&CommandData> {
    match data.expires_for {
        None => Some(data),
        Some(expiration) => {
            if data.created_at.elapsed().unwrap_or(Duration::new(0, 0)) > expiration {
                return None;
            }

            Some(data)
        }
    }
}

fn handle_stream_process(stream_rcp: Arc<Mutex<TcpStream>>) {
    let stream_locked = stream_rcp.lock().unwrap();
    let reader = BufReader::new(&*stream_locked);

    let mut command_queue: Vec<String> = vec![];

    for l in reader.lines() {
        command_queue.push(l.unwrap().to_string());

        if let Some(_command_value) = parse_redis_protocol(&command_queue) {
            command_queue.clear();

            if let Some(key) = &_command_value.param_1 {
                storage_add(
                    key.as_str(),
                    _command_value.param_2.clone().unwrap().as_str(),
                    _command_value.expires_for,
                )
                    .expect("data was saved");
            }

            let mut writer = BufWriter::new(&*stream_locked);

            writer
                .write_all(_command_value.to_response().as_bytes())
                .expect("response was failed");
        }
    }
}

// 0 - param_count
// 1
// 2 - redis_command
// 3
// 4 - value

// *1\r\n$4\r\nPING\r\n
// *2\r\n$4\r\nECHO\r\n$3\r\nhey\r\n
// "*3\r\n$3\r\nSET\r\n$4\r\npear\r\n$6\r\norange\r\n"
// "*5\r\n$3\r\nSET\r\n$4\r\npear\r\n$6\r\norange\r\n$2\r\npx\r\n$3\r\n100\r\n"
// +OK\r\n
// $3\r\nbar\r\n
// $-1\r\n
fn parse_redis_protocol(command_queue: &Vec<String>) -> Option<RedisCommandValue> {
    if command_queue.len() < 3 {
        return None;
    }

    let params_count = command_queue
        .first()
        .unwrap()
        .split('*')
        .collect::<String>()
        .parse::<usize>()
        .unwrap();

    match params_count {
        1 => match command_queue.as_slice() {
            [_, _, command] => Some(RedisCommandValue::new(
                RedisCommand::from_str(command).unwrap(),
                None,
                None,
                None,
            )),
            _ => None,
        },
        2 => match command_queue.as_slice() {
            [_, _, command, _, value] => Some(RedisCommandValue::new(
                RedisCommand::from_str(command).unwrap(),
                None,
                Some(value.to_string()),
                None,
            )),
            _ => None,
        },
        3 => match command_queue.as_slice() {
            [_, _, command, _, key, _, value] => Some(RedisCommandValue::new(
                RedisCommand::from_str(command).unwrap(),
                Some(key.to_string()),
                Some(value.to_string()),
                None,
            )),
            _ => None,
        },
        5 => match command_queue.as_slice() {
            [_, _, command, _, key, _, value, _, _, _, expired    ] => {
                println!("command_queue: {:?}", command_queue);
                Some(RedisCommandValue::new(
                    RedisCommand::from_str(command).unwrap(),
                    Some(key.to_string()),
                    Some(value.to_string()),
                    Some(Duration::from_millis(expired.parse::<usize>().unwrap() as u64)),
                ))
            }
            _ => None,
        },
        _ => None,
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    let mut handles = vec![];

    for stream in listener.incoming() {
        let stream = stream.expect("Unable to accept");
        let stream_rcp = Arc::new(Mutex::new(stream));

        let handle = thread::spawn(move || {
            handle_stream_process(Arc::clone(&stream_rcp));
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
