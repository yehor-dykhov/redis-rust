use std::io::{Read, Write};
use std::net::TcpListener;
use std::str;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let buf = &mut [0; 14];
                let _ = &_stream.read(buf).unwrap();
                let request_txt = str::from_utf8(buf).unwrap();

                if request_txt == "*1\r\n$4\r\nPING\r\n" {
                    _stream.write_all(b"+PONG\r\n").unwrap()
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
