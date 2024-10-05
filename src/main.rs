use itertools::Itertools;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
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
                    Some(r) => r.stream.write_all(&r.response.as_bytes()).ok(),
                    None => Some(()),
                };
                stream.shutdown(Shutdown::Both).ok();
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

pub struct HTTP<'a, T>
where
    T: Into<Vec<u8>>,
{
    // request: Request,
    response: Response<T>,
    stream: &'a mut TcpStream,
}

fn process_req(stream: &mut TcpStream) -> Option<HTTP<'_, impl Into<Vec<u8>>>> {
    let bytes = read_bytes(stream);
    let string = bytes_to_str(bytes).ok();
    let request = parse_req(string);

    if let Some(request) = request {
        match (request.path.path.as_str(), &request.method) {
            ("/", Method::Get) => Some(HTTP {
                // request,
                response: Response {
                    content_length: 0,
                    value: OK.to_owned(),
                },
                stream,
            }),
            ("echo", Method::Get) => {
                let body = request.path.params.join("");
                let len = body.len();

                // TODO: Build response builder that uses content_length
                let res = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    len, body
                );
                Some(HTTP {
                    response: Response {
                        content_length: len,
                        value: res,
                    },
                    stream,
                })
            }
            ("user-agent", Method::Get) => match request.headers().get("User-Agent") {
                None => None,
                Some(v) => {
                    let len = v.as_bytes().len();
                    let body = v;

                    let res = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                            len, body
                        );

                    Some(HTTP {
                        response: Response {
                            content_length: len,
                            value: res,
                        },
                        stream,
                    })
                }
            },
            _ => Some(HTTP {
                // request,
                response: Response {
                    content_length: 0,
                    value: NOT_FOUND.to_owned(),
                },
                stream,
            }),
        }
    } else {
        None
    }
}

pub struct Response<T>
where
    T: Into<Vec<u8>>,
{
    content_length: usize,
    value: T, // bytes: Vec<u8>,
}

impl<T> Response<T>
where
    T: Into<Vec<u8>>,
{
    pub fn as_bytes(self) -> Vec<u8> {
        self.value.into()
    }
}

#[derive(Debug)]
struct Request {
    method: Method,
    path: Path,
    header: Vec<String>,
}

impl Request {
    fn headers(&self) -> HashMap<String, String> {
        self.header
            .iter()
            .fold(HashMap::<String, String, _>::new(), |mut acc, header| {
                if let Some((name, value)) = header.split_once(":") {
                    acc.insert(name.to_owned(), value.to_owned());
                }

                acc
            })
    }
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

                    rest.map(|(headers, _body)| {
                        let header = headers.iter().copied().map(|s| s.to_owned()).collect();

                        Request {
                            method: rp.method,
                            path: rp.path,
                            header,
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
