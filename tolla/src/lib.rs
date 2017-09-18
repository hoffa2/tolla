#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_service;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate prost;
extern crate chrono;
extern crate bytes;
#[macro_use]
extern crate prost_derive;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

// Public modules
pub mod consent;
pub mod proxy;

// Private modules
mod register;
