use itertools::Itertools;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    string::FromUtf8Error,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

const NOT_FOUND: &str = "HTTP/1.1 404 Not Found\r\n\r\n";
const OK: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\n\r\n";

fn main() {
    println!("Logs from your program will appear here!");

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                pool.execute(move || {
                    match process_req(&mut stream) {
                        Some(r) => r.stream.write_all(&r.response.as_bytes()).ok(),
                        None => Some(()),
                    };
                    stream.shutdown(Shutdown::Both).ok();
                });
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Worker {
    id: usize,
    thread: JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = std::thread::spawn(move || loop {
            // This code that uses let job = receiver.lock().unwrap().recv().unwrap();  (or
            // whatever variance that gets dropped after the ';')
            // works because with let, any temporary values used in the expression
            // on the right hand side of the equals sign are immediately dropped when the let statement ends.
            // However, while let (and if let and match) does not drop temporary values
            // until the end of the associated block. In Listing 20-21,
            // the lock remains held for the duration of the call to job(),
            // meaning other workers cannot receive jobs.

            let job = receiver.lock().ok().and_then(|e| e.recv().ok());

            match job {
                Some(job) => {
                    println!("Worker {id} got a job; executing.");

                    job();
                }
                None => println!("Unable to run job on worker {id}"),
            }
        });
        Self { id, thread }
    }
}

struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    fn new(size: usize) -> Self {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        Self { workers, sender }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).ok();
    }
}

struct Http<'a, T>
where
    T: Into<Vec<u8>>,
{
    // request: Request,
    response: Response<T>,
    stream: &'a mut TcpStream,
}

fn process_req(stream: &mut TcpStream) -> Option<Http<'_, impl Into<Vec<u8>>>> {
    let bytes = read_bytes(stream);
    let string = bytes_to_str(bytes).ok();
    let request = parse_req(string);

    if let Some(request) = request {
        match (request.path.path.as_str(), &request.method) {
            ("/", Method::Get) => Some(Http {
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
                Some(Http {
                    response: Response {
                        content_length: len,
                        value: res,
                    },
                    stream,
                })
            }
            ("user-agent", Method::Get) => match request.headers().get("User-Agent") {
                None => Some(Http {
                    response: Response {
                        content_length: 0,
                        value: NOT_FOUND.to_owned(),
                    },
                    stream,
                }),
                Some(v) => {
                    let len = v.as_bytes().len();
                    let body = v;

                    let res = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                            len, body
                        );

                    Some(Http {
                        response: Response {
                            content_length: len,
                            value: res,
                        },
                        stream,
                    })
                }
            },
            _ => Some(Http {
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

struct Response<T>
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
    fn as_bytes(self) -> Vec<u8> {
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
                    acc.insert(name.trim().to_owned(), value.trim().to_owned());
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
struct Path {
    path: String,
    params: Vec<String>,
}
