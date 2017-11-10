use futures::{Future, Stream, Poll};
use std::sync::Arc;
use std::net::{Shutdown, SocketAddr};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::io::{copy, shutdown};
use std::io::{self, Read, Write};
use tokio_core::net::{TcpStream, TcpListener};
use tokio_core::reactor::Core;

pub struct RequestProxy {
    router: Arc<DBrouter>,
}

pub trait DBrouter {
    fn db_route_address(&self, id: &String) -> Result<SocketAddr, String>;
}

impl RequestProxy {
    pub fn new(router: Arc<DBrouter>) -> RequestProxy {
        RequestProxy { router: router }
    }

    pub fn start(&mut self, address: String, port: u16) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let addr = format!("{}:{}", address, port).parse().unwrap();
        let socket = TcpListener::bind(&addr, &handle).unwrap();

        let done = socket.incoming().for_each(move |(client, client_addr)| {
            let remote_addr = self.router
                .db_route_address(&format!("{:?}", client_addr))
                .unwrap();
            let server = TcpStream::connect(&remote_addr, &handle);
            let amounts = server.and_then(move |server| {

                let client_reader = MyTcpStream(Arc::new(client));
                let client_writer = client_reader.clone();
                let server_reader = MyTcpStream(Arc::new(server));
                let server_writer = server_reader.clone();

                let client_to_server =
                    copy(client_reader, server_writer).and_then(|(n, _, server_writer)| {
                        shutdown(server_writer).map(move |_| n)
                    });

                let server_to_client =
                    copy(server_reader, client_writer).and_then(|(n, _, client_writer)| {
                        shutdown(client_writer).map(move |_| n)
                    });

                client_to_server.join(server_to_client)
            });

            let msg = amounts
                .map(move |(from_client, from_server)| {
                    println!(
                        "client at {} wrote {} bytes and received {} bytes",
                        client_addr,
                        from_client,
                        from_server
                    );
                })
                .map_err(|e| {
                    // Don't panic. Maybe the client just disconnected too soon.
                    println!("error: {}", e);
                });
            handle.spawn(msg);

            Ok(())
        });
        core.run(done).unwrap();
    }
}

#[derive(Clone)]
struct MyTcpStream(Arc<TcpStream>);

impl Read for MyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl Write for MyTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncRead for MyTcpStream {}

impl AsyncWrite for MyTcpStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        try!(self.0.shutdown(Shutdown::Write));
        Ok(().into())
    }
}
