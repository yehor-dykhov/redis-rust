use std::cmp::PartialEq;
use std::net::{TcpListener, TcpStream};
use std::str;
use thiserror::Error;
use std::io::{Write, BufReader, BufRead, BufWriter};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Error, Debug)]
pub enum RedisCommandError {
    #[error("invalid command")]
    Invalid(),
    #[error("unknown command")]
    Unknown,
}

#[derive(Error, Debug)]
pub enum RedisResponseCommandError {
    #[error("invalid response command")]
    Invalid(),
    #[error("unknown command")]
    Unknown,
}

#[derive(PartialEq)]
enum RedisCommand {
    PING,
}

impl FromStr for RedisCommand {
    type Err = RedisCommandError;

    fn from_str(s: &str) -> Result<RedisCommand, Self::Err> {
        match s {
            "PING" => Ok(RedisCommand::PING),
            _ => Err(RedisCommandError::Invalid()),
        }
    }
}

#[derive(PartialEq)]
enum RedisCommandResponse {
    PONG,
}

impl RedisCommandResponse {
    fn to_string(self) -> String {
        match self {
            RedisCommandResponse::PONG => "+PONG\r\n".to_owned(),
        }
    }
}

fn handle_stream_process (stream_rcp: Arc<Mutex<TcpStream>>) {
    let stream_locked = stream_rcp.lock().unwrap();
    let reader = BufReader::new(&*stream_locked);

    for l in reader.lines() {
        if let Ok(_line) = RedisCommand::from_str(l.unwrap().as_str()) {
            let mut writer = BufWriter::new(&*stream_locked);
            writer.write(RedisCommandResponse::PONG.to_string().as_bytes()).expect("response was failed");
        }
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
