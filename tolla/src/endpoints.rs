use std::sync::{Arc, Mutex};
use std::io::Read;
use iron::prelude::*;
use iron::Handler;
use router::Router;
use std::str;
use serde_json;
use iron::status::Status;
use consent::ConsentEngine;

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub id: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Query {
    pub user: String,
    pub category: String,
    pub dbname: String,
    pub sql: Vec<String>,
}

pub struct Handlers {
    pub dbquery: QueryHandler,
    pub register: Register,
    pub remove: Remove,
}

impl Handlers {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> Handlers {
        Handlers {
            dbquery: QueryHandler::new(router.clone()),
            register: Register::new(router.clone()),
            remove: Remove::new(router.clone()),
        }
    }
}

pub struct QueryHandler {
    router: Arc<Mutex<ConsentEngine>>,
}

pub struct Register {
    router: Arc<Mutex<ConsentEngine>>,
}

pub struct Remove {
    router: Arc<Mutex<ConsentEngine>>,
}

impl QueryHandler {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> QueryHandler {
        QueryHandler { router: router }
    }
}

impl Handler for QueryHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let user = req.extensions
            .get::<Router>()
            .unwrap()
            .find("user")
            .unwrap_or("/");

        let router = self.router.clone();
        let ip = match router.lock().unwrap().consent_based_view(
            &String::from(user),
        ) {
            Ok(ip) => ip,
            Err(err) => return Ok(Response::with((Status::BadRequest, err.to_string()))),
        };
        Ok(Response::with((Status::Ok, ip)))
    }
}

impl Register {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> Register {
        Register { router: router }
    }
}

impl Handler for Register {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let mut raw = String::new();

        if let Err(e) = req.body.read_to_string(&mut raw) {
            return Ok(Response::with((Status::BadRequest, e.to_string())));
        }

        let deserialized: User = match serde_json::from_str(&raw) {
            Ok(d) => d,
            Err(e) => return Ok(Response::with((Status::BadRequest, e.to_string()))),
        };

        let router = self.router.clone();
        if let Err(err) = router.lock().unwrap().deboard_user(&deserialized.id) {
            return Ok(Response::with((Status::BadRequest, err.to_string())));
        };
        Ok(Response::with(Status::Ok))
    }
}

impl Remove {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> Remove {
        Remove { router: router }
    }
}

impl Handler for Remove {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let user = req.extensions
            .get::<Router>()
            .unwrap()
            .find("user")
            .unwrap_or("/");

        let router = self.router.clone();
        if let Err(err) = router.lock().unwrap().deboard_user(&String::from(user)) {
            return Ok(Response::with((Status::BadRequest, err.to_string())));
        };
        Ok(Response::with(Status::Ok))
    }
}
