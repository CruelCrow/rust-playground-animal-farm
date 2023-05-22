use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write};
use std::{thread, time};
use std::fs::{self, OpenOptions, File};
use simple_websockets::{Event, Responder, Message};
use std::collections::HashMap;
use serde_json::{Result, Value};
use rand::Rng;

const ANIMAL_FILE: &str = "/tmp/current.animal.txt";

#[derive(Debug)]
struct Request {
    method: String,
    path: String
}


fn handle_read(mut stream: &TcpStream) -> Request {
    let mut buf = [0u8 ;4096];
    match stream.read(&mut buf) {
        Ok(_) => {
            let req_str = String::from_utf8_lossy(&buf);
            println!("{}", req_str);
            parse_req(&req_str)
            },
        Err(e) => {
            println!("Unable to read stream: {}", e);
            Request {
                method: String::from("GET"),
                path: String::from("500")
            }
        }
    }
}

fn parse_req(req_str: &str) -> Request {
    let mut lines = req_str.lines();
    let first_line = lines.next().unwrap();
    let mut parts = first_line.split(" ");
    Request {
        method: parts.next().unwrap().to_string(),
        path: parts.next().unwrap().to_string()
    }
}

fn handle_write(mut stream: TcpStream, req: &Request) {
    if req.method == "GET" && req.path == "/" {
        let http_prefix = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8";
        let index = fs::read_to_string("res/index.html").unwrap();
        let response = [http_prefix, &index].join("\r\n\r\n");

        stream.write(response.as_bytes());
    } else if req.method == "GET" && req.path == "/animals.json" {
        let http_prefix = "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=UTF-8";
        let json = fs::read_to_string("res/animals.json").unwrap();
        let response = [http_prefix, &json].join("\r\n\r\n");

        stream.write(response.as_bytes());
    } else if req.method == "PUT" && req.path.starts_with("/animal:") {
        let animal = req.path.get(8..).unwrap();

        let mut file = OpenOptions::new().write(true).truncate(true).open(ANIMAL_FILE).unwrap();
        file.write(animal.as_bytes());

        let http_prefix = "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=UTF-8";
        let response = [http_prefix, &animal].join("\r\n\r\n");

        stream.write(response.as_bytes());
    } else {
        stream.write(b"HTTP/1.1 404 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\nPage not found");
    }
}

fn handle_client(stream: TcpStream) {
    let req = handle_read(&stream);
    println!("{:?}", req);
    handle_write(stream, &req);
}

fn get_animal_voice() -> String {
    let current_animal = fs::read_to_string(ANIMAL_FILE).unwrap();
    let json = fs::read_to_string("res/animals.json").unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    let mut rng = rand::thread_rng();
    let voice = v[current_animal][rng.gen_range(0..3)].to_string();
    if (voice == "null") {
        String::from("")
    } else {
        voice
    }
}

fn main() {
    File::create(ANIMAL_FILE);

    let handle_http = thread::spawn(move|| {

        let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
        println!("Listening for connections on port {}", 8080);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_client(stream)
                }
                Err(e) => {
                    println!("Unable to connect: {}", e);
                }
            }
        }
    });
    
    let handle_animal = thread::spawn(move || {
        let event_hub = simple_websockets::launch(8081).expect("failed to listen on port 8081");
        let mut clients: HashMap<u64, Responder> = HashMap::new();
        loop {
            match event_hub.poll_event() {
                Event::Connect(client_id, responder) => {
                    println!("A client connected with id #{}", client_id);
                    clients.insert(client_id, responder.clone());
                    thread::spawn(move || {
                        loop {
                            let current_animal = fs::read_to_string(ANIMAL_FILE).unwrap();
                            println!("Current animal: {}", current_animal);

                            let animal_voice = Message::Text(get_animal_voice());
                            responder.send(animal_voice);

                            thread::sleep(time::Duration::from_millis(1000));
                        }
                    });
                },
                Event::Disconnect(client_id) => {
                    println!("Client #{} disconnected.", client_id);
                    clients.remove(&client_id);
                },
                Event::Message(client_id, message) => {
                    println!("Received a message from client #{}: {:?}", client_id, message);
                    let responder = clients.get(&client_id).unwrap();
                    responder.send(message);
                },
            }
        }
    });

    handle_http.join().unwrap();
    handle_animal.join().unwrap();
}
