extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate mysql_proxy;

use mysql_proxy::*;
use futures::{Future, Stream};
use std::net::IpAddr;
use tokio_io::{io, AsyncRead};
use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use rustls::internal::permfile::{certs, rsa_private_keys};
use rustls::{Certificate, PrivateKey, ServerConfig};
use native_tls::TlsConnector;

pub struct RequestProxy {
    pub incoming_address: String,
    pub incoming_port: u16,
    pub router: DBrouter,
}

trait DBrouter {
    fn db_route_address(&self, id: &String) -> Result<IpAddr, String>;
}

impl RequestProxy {
    pub fn start(&self) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let addr = format!("{}:{}", self.incoming_address, self.incoming_port).parse().unwrap();
        let tcp = TcpListener::bind(&addr, &handle).unwrap();
        let mut config = ServerConfig::new();

        let server = tcp.incoming().for_each(|(stream, remote_addr)| {
            let done = arc_config.accept_async(stream).and_then(|stream| {
                let cert = stream.sess.get_peer_certificates();
                let forward_addr = self.router.db_route_address (&String::from("id"));
                
            });

        });

    }
}
