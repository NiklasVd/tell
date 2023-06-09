use std::{io::{stdout, stdin, Write}, str::FromStr, fmt::Debug, net::SocketAddr, sync::{Arc, Mutex}};
use crossbeam_channel::unbounded;
use log::error;
use tell_lib::{net::{adapter::{Rx, AdapterConfig}, server::Server, client::Client}, err::TResult, id::Id, packet::TargetMode};

fn main() -> TResult {
    println!("Tell Client -- v{}", env!("CARGO_PKG_VERSION"));
    simple_logger::init().unwrap();
    let name = read_line("Name");
    let id = Id::new(name)?;
    let mode = read_line("Mode[client/server]");
    let port: u16 = read_input("Port");
    if mode == "server" {
        server(id, port)
    } else if mode == "client" {
        let target_addr: SocketAddr = read_input("Target Address [ip:port]");
        client(id, port, target_addr)
    } else {
        panic!("Invalid mode")
    }
}

fn server(id: Id, port: u16) -> TResult {
    let server = Server::setup(id, AdapterConfig {
        port, max_conns: 16
    })?;
    let server = Arc::new(Mutex::new(server));
    let poll_server = server.clone();
    std::thread::spawn(move || {
        loop {
            if let Err(e) = poll_server.lock().unwrap().poll() {
                error!("Poll thread err: {e}.")
            }
        }
    });
    loop {
        let cmd = read_line("Cmd");
        if cmd == "metrics" {
            server.lock().unwrap().print_metrics();
        }
    }
}

fn client(id: Id, port: u16, target_addr: SocketAddr) -> TResult {
    let mut client = Client::new(id, port)?;
    client.connect(target_addr)?;
    let client = Arc::new(Mutex::new(client));
    let poll_client = client.clone();
    std::thread::spawn(move || {
        loop {
            if let Err(e) = poll_client.lock().unwrap().poll() {
                error!("Poll thread err: {e}.")
            }
        }
    });
    loop {
        let cmd = read_line("Cmd [msg/metrics]");
        if cmd == "msg" {
            let msg = read_line("Write");
            if !msg.is_empty() {
                client.lock().unwrap().message(TargetMode::Broadcast, msg)?;
            }
        } else if cmd == "metrics" {
            client.lock().unwrap().print_metrics();
        }
    }
}

pub fn read_line(input: &str) -> String {
    let mut line = String::new();
    print!("{}/: ", input);
    stdout().flush().expect("Failed to flush stream");

    stdin().read_line(&mut line).expect("Failed to read line");
    line.trim().to_owned()
}

pub fn read_input<T: FromStr + Debug>(input: &str) -> T where <T as FromStr>::Err: Debug {
    match read_line(input.clone()).parse::<T>() {
        Ok(n) => n,
        Err(e) => {
            println!("{:?}", e);
            read_input(input)
        }
    }
}

fn spawn_input_thread() -> Rx<String> {
    let (tx, rx) = unbounded();
    std::thread::spawn(move || loop {
        let text = read_line("Write");
        if let Err(e) = tx.send(text) {
            println!("Input thread encountered err: {e}.");
            break
        }
    });
    rx
}

