use std::borrow::ToOwned;
use std::cmp::PartialEq;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::str;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
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

#[derive(PartialEq)]
enum RedisCommand {
    Ping,
    Echo,
}

impl FromStr for RedisCommand {
    type Err = RedisCommandError;

    fn from_str(s: &str) -> Result<RedisCommand, Self::Err> {
        match s {
            "PING" => Ok(RedisCommand::Ping),
            "ECHO" => Ok(RedisCommand::Echo),
            _ => Err(RedisCommandError::Invalid(s.to_string())),
        }
    }
}

struct RedisCommandValue {
    command: RedisCommand,
    value: Option<String>,
}

impl RedisCommandValue {
    fn new(command: RedisCommand, value: Option<String>) -> Self {
        Self { command, value }
    }

    fn to_response(&self) -> String {
        match self.command {
            RedisCommand::Ping => "+PONG\r\n".to_owned(),
            RedisCommand::Echo => {
                let len = self.value.clone().unwrap_or("".to_string()).len();
                format!("${len}\r\n{}\r\n", self.value.as_ref().unwrap())
            }
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

    match command_queue.as_slice() {
        [_, _, command] => {
            if params_count == 1 {
                Some(RedisCommandValue::new(
                    RedisCommand::from_str(command).unwrap(),
                    None,
                ))
            } else {
                None
            }
        }
        [_, _, command, _, value] => Some(RedisCommandValue::new(
            RedisCommand::from_str(command).unwrap(),
            Some(value.to_string()),
        )),
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
