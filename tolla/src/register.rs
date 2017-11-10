use futures::{future, Future};
use tokio_service::Service;
use std::io;
use tolla_proto::proto;
use std::sync::{Arc, Mutex};
use consent::ConsentEngine;

pub struct ProtoService {
    pub engine: Arc<Mutex<ConsentEngine>>,
}

impl Service for ProtoService {
    type Request = proto::FromClient;
    type Response = proto::ToClient;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let engine = self.engine.clone();
        let resp = engine.lock().unwrap().handle_incoming(req);
        match resp {
            Ok(r) => return Box::new(future::ok(r)),
            Err(err) => {
                return Box::new(future::err(
                    io::Error::new(io::ErrorKind::InvalidData, err.to_string()),
                ))
            }
        }
    }
}
