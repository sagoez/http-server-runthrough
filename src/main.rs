// Uncomment this block to pass the first stage
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                stream.write_all("HTTP/1.1 200 OK\r\n\r\n".as_bytes()).ok();
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

fn read_bytes(stream: &mut TcpStream) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut bytes_read = 0;
    loop {
        let mut buf = [0; 1024];
        match stream.read(&mut buf) {
            Ok(n) => {
                bytes_read += n;
                buffer.extend_from_slice(&buf[..n]);
                if bytes_read == buffer.len() {
                    break;
                }
            }
            Err(e) => {
                println!("error: {}", e);
                break;
            }
        }
    }

    buffer
}
