// Uncomment this block to pass the first stage
use itertools::Itertools;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    string::FromUtf8Error,
};

const NOT_FOUND: &str = "HTTP/1.1 404 Not Found\r\n\r\n";
const OK: &str = "HTTP/1.1 200 OK\r\n\r\n";

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                //TODO: Temporarily making it all optional
                let bytes = read_bytes(&mut stream);
                let string = bytes_to_str(bytes).ok();
                let request = parse_req(string);

                if let Some(request) = request {
                    if request.method.path() == "/" {
                        stream.write_all(OK.as_bytes()).ok();
                    } else {
                        stream.write_all(NOT_FOUND.as_bytes()).ok();
                    }
                };
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

pub struct Request {
    method: Method,
    headers: Vec<String>,
}

// TODO: Split headers and take body into account
fn parse_req(str: Option<String>) -> Option<Request> {
    match str {
        Some(str) => match str.split("\r\n").collect_vec().split_first() {
            Some((first, elements)) => {
                let method = Method::from_static(first);
                method.and_then(|method| {
                    let rest = elements.split(|s| s == &"\r\n\r\n").collect_vec();

                    let rest = rest.split_first();

                    rest.map(|(headers, _body)| Request {
                        method,
                        headers: headers.iter().copied().map(|s| s.to_owned()).collect(),
                    })
                })
            }
            None => None,
        },
        None => None,
    }
}

fn bytes_to_str(vec: Vec<u8>) -> Result<String, FromUtf8Error> {
    String::from_utf8(vec)
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

pub enum Method {
    Get {
        path: String,
        http: String, // Either V1, or V2
    },
}

impl Method {
    pub fn path(&self) -> String {
        match self {
            Method::Get { path, .. } => path.into(),
        }
    }

    pub fn from_static(str: &str) -> Option<Method> {
        let splitted = str.split(" ").collect_vec();
        // Extract URL path
        if splitted.len() != 3 {
            None
        } else {
            // TODO: Generalize this
            if splitted[0] == "GET" {
                Some(Method::Get {
                    path: splitted[1].to_owned(),
                    http: splitted[2].to_owned(),
                })
            } else {
                None
            }
        }
    }
}
