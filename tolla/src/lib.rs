#![deny(warnings)]

#[macro_use(bson, doc)]
extern crate bson;
extern crate mongodb;
extern crate futures;
#[macro_use]
extern crate log;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde;
extern crate prost;
extern crate mysql;
extern crate chrono;
extern crate iron;
extern crate router;
extern crate jwt;
extern crate shiplift;
extern crate url;
extern crate uuid;
extern crate openssl;
extern crate tolla_proto;

extern crate bytes;


// Public modules
pub mod consent;
pub mod proxy;
pub mod endpoints;
mod ca;

// Private modules
pub mod register;
mod docker;
