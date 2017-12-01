use std::sync::{Arc, Mutex};
use std::io::Read;
use iron::prelude::*;
use iron::Handler;
use router::Router;
use std::str;
use serde_json;
use iron::status::Status;
use consent::ConsentEngine;
use urlencoded::UrlEncodedQuery;

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub id: String,
    pub purposes: Vec<String>,
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
    pub lease: Lease,
}

impl Handlers {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> Handlers {
        Handlers {
            dbquery: QueryHandler::new(router.clone()),
            register: Register::new(router.clone()),
            remove: Remove::new(router.clone()),
            lease: Lease::new(router.clone()),
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

pub struct Lease {
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
        if let Err(err) = router.lock().unwrap().onboard_user(
            &deserialized.id,
            deserialized.purposes,
        )
        {
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



impl Lease {
    pub fn new(router: Arc<Mutex<ConsentEngine>>) -> Lease {
        Lease { router: router }
    }
}

impl Handler for Lease {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {

        let query_params = match req.get_ref::<UrlEncodedQuery>() {
            Ok(hashmap) => hashmap,
            Err(err) => return Ok(Response::with((Status::BadRequest, err.to_string()))),
        };

        let user = match query_params.get("user") {
            Some(user) => user,
            None => {
                return Ok(Response::with(
                    (Status::BadRequest, "No user in query".to_string()),
                ))
            }
        };

        let intent = match query_params.get("intent") {
            Some(intent) => intent,
            None => {
                return Ok(Response::with(
                    (Status::BadRequest, "No intent in query".to_string()),
                ))
            }
        };

        let serial_number = match user[0].parse::<u32>() {
            Err(err) => return Ok(Response::with((Status::BadRequest, err.to_string()))),
            Ok(serial_number) => serial_number,
        };

        let router = self.router.clone();
        let consent = match router.lock().unwrap().consent_by_serial_num(serial_number) {
            Err(err) => return Ok(Response::with((Status::BadRequest, err.to_string()))),
            Ok(consent) => consent,
        };

        match consent.purpose.contains(&intent[0]) {
            true => return Ok(Response::with(Status::Ok)),
            false => {
                return Ok(Response::with(
                    (Status::Forbidden, format!("Consents did not match")),
                ))
            }
        }
    }
}
