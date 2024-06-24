use std::cmp::PartialEq;
use std::net::{TcpListener, TcpStream};
use std::str;
use thiserror::Error;
use std::io::{Write, BufReader, BufRead, BufWriter};
use std::str::FromStr;

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

fn handle_stream_process (tcp_stream: TcpStream) {
    let reader = BufReader::new(&tcp_stream);

    for l in reader.lines() {
        if let Ok(_line) = RedisCommand::from_str(l.unwrap().as_str()) {
            let mut writer = BufWriter::new(&tcp_stream);
            writer.write(RedisCommandResponse::PONG.to_string().as_bytes()).expect("response was failed");
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        let stream = stream.expect("Unable to accept");

        handle_stream_process(stream);
    }
}
