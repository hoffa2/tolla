use futures::{future, Future};
use tokio_service::Service;
use bytes::{Bytes, Buf, IntoBuf};
use prost::Message;

use std::io;
use prost::encoding::message;
use proto;
use consent;

impl Service for consent::ConsentEngine {
    type Request = Vec<u8>;
    type Response = Vec<u8>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let len = req.len();
        let msg = proto::FromClient::decode(&mut Buf::take(req.into_buf(), len));
        if let Err(err) = msg {
            return Box::new(future::err(io::Error::new(io::ErrorKind::Other, err)));
        }

        let resp = self.handle_incoming(msg.unwrap());
        Box::new(future::ok(resp))
    }
}


