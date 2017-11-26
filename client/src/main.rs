extern crate config;
extern crate futures;
extern crate serde;
extern crate prost;
extern crate tokio_proto;
extern crate tolla_proto;
extern crate tokio_core;
extern crate openssl;
extern crate native_tls;
extern crate tokio_io;
extern crate bytes;
#[macro_use]
extern crate serde_derive;
#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;

use prost::Message;
use tokio_core::net::TcpStream;
use tolla_proto::proto;
use openssl::x509::X509ReqBuilder;
use openssl::x509::X509NameBuilder;
use openssl::rsa::Rsa;
use tokio_io::AsyncRead;
use openssl::pkey::PKey;
use openssl::hash::MessageDigest;
use tokio_core::reactor::Core;
use futures::{Future, Sink, Stream};
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::fs::File;
use bson::Bson;
use mongodb::{Client, ClientOptions, ThreadedClient};
use mongodb::db::ThreadedDatabase;
use std::env;

#[derive(Debug, Deserialize)]
pub struct Ca {
    address: String,
    port: String,
}

#[derive(Debug, Deserialize)]
pub struct Process {
    common_name: String,
    country: String,
    intent: String,
    state: String,
    org: String,
}

#[derive(Debug, Deserialize)]
pub struct Certs {
    key: String,
    ca: String,
    cert: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    process: Process,
    ca: Ca,
    certs: Certs,
}

struct TollaClient {
    core: Core,
}

fn dump_certificate(filename: &str, data: &Vec<u8>) {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(filename)
        .unwrap();

    file.write_all(&data).unwrap();
}

fn setup_mongod(certs: &Certs, address: String) {
    let options = ClientOptions::with_ssl(&certs.ca, &certs.cert, &certs.key, true);

    let addr = env::var("DB_ADDRESS").unwrap();

    println!("Connecting to {}", addr);

    let client = Client::connect_with_options(&addr, 8080, options).unwrap();
    println!("came here");
    let views = client.db("test").collection("view");
    println!("came here2");

    let doc =
        doc! {
        "title": "Jaws",
        "array": [ 1, 2, 3 ],
    };
    println!("came here3");

    if let Err(e) = views.insert_one(doc.clone(), None) {
        println!("{}", e.to_string());
    }
    println!("came here 4");
}

fn main() {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("settings")).unwrap();

    let mut tolla_client = TollaClient { core: Core::new().unwrap() };

    let conf = settings.try_into::<Settings>().unwrap();
    let mut buf = Vec::new();

    if let Err(_) = File::open("cert.pem") {
        tolla_client.retrieve_certificate(&conf, &mut buf).unwrap();
    }

    let mut msg = proto::FromClient::default();
    msg.msg = Some(proto::from_client::Msg::Requestips(true));

    let socket_addr: SocketAddr = format!("{}:{}", conf.ca.address, conf.ca.port)
        .parse()
        .unwrap();

    let result = tolla_client.send_protorequest(&socket_addr, msg);

    if let Some(proto::to_client::Msg::Ips(addr)) = result.unwrap().msg {
        setup_mongod(&conf.certs, addr.ip[0].clone());
    };
}

impl TollaClient {
    fn retrieve_certificate(&mut self, conf: &Settings, buf: &mut Vec<u8>) -> Result<(), String> {
        let keypair = match Rsa::generate(1024) {
            Ok(keypair) => keypair,
            Err(err) => return Err(err.to_string()),
        };

        let pkey = match PKey::from_rsa(keypair) {
            Ok(pkey) => pkey,
            Err(err) => return Err(err.to_string()),
        };

        let certificate = self.issue_cert_request(&conf.ca, &conf.process, &pkey)?;

        if let proto::to_client::Msg::Certificate(c) = certificate.msg.unwrap() {
            dump_certificate("cert.pem", &c.request);
            dump_certificate("ca.pem", &c.root_cert);
        }

        let pkey_pem = pkey.private_key_to_pem().map_err(|e| e.to_string())?;

        dump_certificate("key.pem", &pkey_pem);

        Ok(())
    }
    fn issue_cert_request(
        &mut self,
        cert_auth: &Ca,
        process: &Process,
        pkey: &PKey,
    ) -> Result<proto::ToClient, String> {
        let mut cert = match X509ReqBuilder::new() {
            Ok(cert) => cert,
            Err(err) => return Err(err.to_string()),
        };

        if let Err(err) = cert.set_pubkey(&pkey) {
            return Err(err.to_string());
        }

        let mut x509_name = match X509NameBuilder::new() {
            Ok(x509_name) => x509_name,
            Err(err) => return Err(err.to_string()),
        };

        if let Err(err) = x509_name.append_entry_by_text("C", &process.country) {
            return Err(err.to_string());
        }
        if let Err(err) = x509_name.append_entry_by_text("ST", &process.state) {
            return Err(err.to_string());
        }
        if let Err(err) = x509_name.append_entry_by_text("O", &process.org) {
            return Err(err.to_string());
        }
        if let Err(err) = x509_name.append_entry_by_text("CN", &process.common_name) {
            return Err(err.to_string());
        }

        let x509_name = x509_name.build();

        if let Err(err) = cert.set_subject_name(&x509_name) {
            return Err(err.to_string());
        }

        if let Err(err) = cert.sign(&pkey, MessageDigest::sha256()) {
            return Err(err.to_string());
        }

        let cert = cert.build();

        let pem = match cert.to_pem() {
            Ok(pem) => pem,
            Err(err) => return Err(err.to_string()),
        };

        let socket_addr: SocketAddr = format!("{}:{}", cert_auth.address, cert_auth.port)
            .parse()
            .unwrap();

        let mut msg = proto::FromClient::default();
        msg.msg = Some(proto::from_client::Msg::Certificaterequest(
            proto::Certificate {
                intent: process.intent.clone(),
                request: pem.clone(),
                root_cert: Vec::new(),
            },
        ));

        self.send_protorequest(&socket_addr, msg)
    }

    fn send_protorequest(
        &mut self,
        addr: &SocketAddr,
        req: proto::FromClient,
    ) -> Result<proto::ToClient, String> {
        let handle = self.core.handle();

        let work = TcpStream::connect(addr, &handle).and_then(|socket| {
            let transport = socket.framed(proto::ProtoClient);
            transport.send(req).and_then(
                |socket| socket.take(1).collect(),
            )
        });

        let resp = self.core.run(work);
        println!("{:?}", resp);
        match resp {
            Ok(resp) => return Ok(resp[0].clone()),
            Err(err) => return Err(err.to_string()),
        }
    }
}
