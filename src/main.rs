use std::borrow::ToOwned;
use std::cmp::PartialEq;
use std::collections::HashMap;
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
        match s {
            "PING" => Ok(RedisCommand::Ping),
            "ECHO" => Ok(RedisCommand::Echo),
            "SET" => Ok(RedisCommand::Set),
            "GET" => Ok(RedisCommand::Get),
            _ => Err(RedisCommandError::Invalid(s.to_string())),
        }
    }
}

#[derive(Debug)]
struct RedisCommandValue {
    command: RedisCommand,
    key: Option<String>,
    value: Option<String>,
}

impl RedisCommandValue {
    fn new(command: RedisCommand, key: Option<String>, value: Option<String>) -> Self {
        Self {
            command,
            key,
            value,
        }
    }

    fn to_response(&self, store: Arc<Mutex<HashMap<String, String>>>) -> String {
        match self.command {
            RedisCommand::Ping => "+PONG\r\n".to_owned(),
            RedisCommand::Set => "+OK\r\n".to_owned(),
            RedisCommand::Get => {
                if let Some(v) = read_store(store, self.value.clone().unwrap().as_str()) {
                    let len = v.len();
                    return format!("${len}\r\n{}\r\n", v);
                }

                "$-1\r\n".to_string()
            }
            RedisCommand::Echo => {
                let len = self.value.clone().unwrap_or("".to_string()).len();
                format!("${len}\r\n{}\r\n", self.value.as_ref().unwrap())
            }
        }
    }
}

fn write_store(
    _store: Arc<Mutex<HashMap<String, String>>>,
    redis_command_value: &RedisCommandValue,
) -> bool {
    match redis_command_value.command {
        RedisCommand::Set => {
            (*_store).lock().unwrap().insert(
                redis_command_value.key.clone().unwrap(),
                redis_command_value.value.clone().unwrap(),
            );
            true
        }
        _ => false,
    }
}

fn read_store(_store: Arc<Mutex<HashMap<String, String>>>, key: &str) -> Option<String> {
    _store.lock().unwrap().get(key).cloned()
}

fn handle_stream_process(
    stream_rcp: Arc<Mutex<TcpStream>>,
    _store: Arc<Mutex<HashMap<String, String>>>,
) {
    let stream_locked = stream_rcp.lock().unwrap();
    let reader = BufReader::new(&*stream_locked);

    let mut command_queue: Vec<String> = vec![];

    for l in reader.lines() {
        command_queue.push(l.unwrap().to_string());

        if let Some(_command_value) = parse_redis_protocol(&command_queue) {
            command_queue.clear();
            write_store(_store.clone(), &_command_value);
            let mut writer = BufWriter::new(&*stream_locked);

            writer
                .write_all(_command_value.to_response(_store.clone()).as_bytes())
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
            )),
            _ => None,
        },
        2 => match command_queue.as_slice() {
            [_, _, command, _, value] => Some(RedisCommandValue::new(
                RedisCommand::from_str(command).unwrap(),
                None,
                Some(value.to_string()),
            )),
            _ => None,
        },
        3 => match command_queue.as_slice() {
            [_, _, command, _, key, _, value] => Some(RedisCommandValue::new(
                RedisCommand::from_str(command).unwrap(),
                Some(key.to_string()),
                Some(value.to_string()),
            )),
            _ => None,
        },
        _ => None,
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    let mut handles = vec![];
    let store: HashMap<String, String> = HashMap::new();
    let redis_store = Arc::new(Mutex::new(store));

    for stream in listener.incoming() {
        let stream = stream.expect("Unable to accept");
        let stream_rcp = Arc::new(Mutex::new(stream));
        let store_copied = Arc::clone(&redis_store);

        let handle = thread::spawn(move || {
            handle_stream_process(Arc::clone(&stream_rcp), store_copied);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
