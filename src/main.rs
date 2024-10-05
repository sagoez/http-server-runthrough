use itertools::Itertools;
use nom::AsBytes;
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
                match process_req(&mut stream) {
                    Some(r) => r.stream.write_all(&r.response.bytes).ok(),
                    None => Some(()),
                };
                // process_req(&mut stream).and_then(|r| stream.write_all(NOT_FOUND.as_bytes()).ok());
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

pub struct HTTP<'a> {
    // request: Request,
    response: Response,
    stream: &'a mut TcpStream,
}

fn process_req(stream: &mut TcpStream) -> Option<HTTP<'_>> {
    let bytes = read_bytes(stream);
    let string = bytes_to_str(bytes).ok();
    let request = parse_req(string);

    if let Some(request) = request {
        match (request.path.path.as_str(), &request.method) {
            ("/", Method::Get) => Some(HTTP {
                // request,
                response: Response {
                    content_lengh: 0,
                    bytes: OK.as_bytes().to_vec(),
                },
                stream,
            }),
            ("echo", Method::Get) => {
                let body = request.path.params.join("");
                let len = body.len();

                // TODO: Build response builder
                let res = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    len, body
                );
                Some(HTTP {
                    // request,
                    response: Response {
                        content_lengh: len,
                        bytes: res.as_bytes().to_vec(),
                    },
                    stream,
                })
            }
            _ => Some(HTTP {
                // request,
                response: Response {
                    content_lengh: 0,
                    bytes: NOT_FOUND.as_bytes().to_vec(),
                },
                stream,
            }),
        }
    } else {
        None
    }
}

pub struct Response {
    content_lengh: usize,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct Request {
    method: Method,
    path: Path,
    #[allow(dead_code)]
    header: Vec<String>,
    body: Vec<u8>,
}

// TODO: Split headers and take body into account
fn parse_req(str: Option<String>) -> Option<Request> {
    match str {
        Some(str) => match str.split("\r\n").collect_vec().split_first() {
            Some((first, elements)) => {
                let request_parts = RequestPart::from_static(first);
                request_parts.and_then(|rp| {
                    let rest = elements.split(|s| s == &"\r\n\r\n").collect_vec();
                    let rest = rest.split_first();

                    rest.map(|(headers, body)| {
                        let header = headers.iter().copied().map(|s| s.to_owned()).collect();

                        Request {
                            method: rp.method,
                            path: rp.path,
                            header,
                            body: flatten_to_bytes(body),
                        }
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

struct RequestPart {
    method: Method,
    path: Path,
}

#[derive(PartialEq, Debug)]
enum Method {
    Get,
}

impl Method {
    fn from_static(str: &str) -> Option<Method> {
        match str {
            "GET" => Some(Method::Get),
            _ => None,
        }
    }
}

impl RequestPart {
    fn from_static(str: &str) -> Option<RequestPart> {
        let request_parts = str.split(" ").collect_vec();

        if let [method, path, _http] = &request_parts[..] {
            let elements = path
                .trim()
                .split_terminator("/")
                .filter(|s| !s.is_empty())
                .collect_vec();

            let path = match elements.split_first() {
                Some((path, params)) => Path {
                    path: path.to_string(),
                    params: params.iter().copied().map(|s| s.to_owned()).collect_vec(),
                },
                None => Path {
                    path: path.to_string(),
                    params: vec![],
                },
            };

            Method::from_static(method).map(|method| RequestPart { method, path })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Path {
    path: String,
    params: Vec<String>,
}

// TODO: Pretty sure this is wrong
fn flatten_to_bytes(body: &[&[&str]]) -> Vec<u8> {
    body.iter()
        .flat_map(|inner| inner.iter().flat_map(|&s| s.as_bytes()))
        .copied()
        .collect()
}
