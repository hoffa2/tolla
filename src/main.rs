#![deny(warnings)]

extern crate lib_tolla;
extern crate tokio_proto;
extern crate tokio_core;
extern crate futures;
extern crate prost;
extern crate router;
extern crate iron;
extern crate tolla_proto;
extern crate log;
extern crate simplelog;

use tokio_proto::TcpServer;
use std::net::TcpStream;
use tolla_proto::proto;
use lib_tolla::*;
use std::io::Write;
use std::io::Read;
use std::env;
use prost::Message;
use std::{thread, time};
use iron::Iron;
use router::Router;
use std::sync::{Arc, Mutex};
use log::LogLevel;
use simplelog::{Config, WriteLogger, LogLevelFilter};
use std::fs::File;

fn main() {
    let log_conf = Config {
        time: Some(LogLevel::Debug),
        level: Some(LogLevel::Debug),
        target: Some(LogLevel::Debug),
        location: Some(LogLevel::Debug),
    };
    WriteLogger::init(
        LogLevelFilter::Trace,
        log_conf,
        File::create("log.log").unwrap(),
    ).unwrap();

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("usage: {} address", args[0]);
        return;
    }

    match args[1].as_ref() {
        "http" => run_http(),
        "client" => run_client(String::from("0.0.0.0"), 8900),
        _ => println!("Wrong"),
    }
}

fn run_http() {
    let consent = consent::ConsentEngineBuilder::new()
        .address(format!("127.0.0.1"))
        .port(27017)
        .deamon(format!("http://127.0.0.1:2375"))
        .build()
        .unwrap();

    let consent_ref = Arc::new(Mutex::new(consent));

    let handlers = endpoints::Handlers::new(consent_ref.clone());

    let mut router = Router::new();

    router.get("/:user", handlers.dbquery, "get");
    router.post("/register", handlers.register, "register");
    router.delete("/:user", handlers.remove, "remove");

    thread::spawn(move || {
        Iron::new(router).http("localhost:3001").unwrap();
    });

    let addr = "0.0.0.0:8900".parse().unwrap();
    let mut server = TcpServer::new(proto::ProtoProto, addr);

    server.threads(8);

    server.serve(move || {
        Ok(register::ProtoService { engine: consent_ref.clone() })
    });
}

fn run_client(addr: String, port: u16) {
    // Sleep such that server has time to get up and running
    thread::sleep(time::Duration::from_millis(100));

    let fmt_addr = format!("{}:{}", addr, port);
    let mut conn = match TcpStream::connect(&fmt_addr) {
        Ok(conn) => conn,
        Err(err) => {
            println!("{}", err.to_string());
            return;
        }
    };

    let mut msg = proto::FromClient::default();
    msg.msg = Some(proto::from_client::Msg::User(proto::NewUser {
        userid: String::from("jonnebassen2"),

        email: String::from("lol"),
    }));
    let mut buf = Vec::new();
    if let Err(err) = msg.encode(&mut buf) {
        println!("{}", err.to_string());
        return;
    }

    if let Err(err) = conn.write(buf.as_slice()) {
        println!("{}", err.to_string());
    }

    let mut resp = Vec::with_capacity(80);
    if let Err(err) = conn.read_to_end(&mut resp) {
        println!("{}", err.to_string());
    }
    println!("{}", resp.len());
}
