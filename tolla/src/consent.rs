use bson;
use mongodb::{Client, ThreadedClient};
use mongodb::db::ThreadedDatabase;
use std::time::Duration;
use tolla_proto::proto;
use chrono::prelude::*;
use std::collections::HashMap;
use bytes::BytesMut;
use ca::Authority;
use docker;
use std::env;

#[derive(Serialize, Deserialize, Debug)]
pub struct Consent {
    #[serde(rename = "_id")]
    pub id: String,
    pub purpose: Vec<String>,
    pub lifetime: Duration,
    pub aquired: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Intent {
    #[serde(rename = "_id")]
    // Container ID
    pub id: String,
    pub intent: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct View {
    #[serde(rename = "_id")]
    pub id: String,
    pub ip: String,
}

pub struct ConsentEngineBuilder {
    address: Option<String>,
    port: Option<u16>,
    deamon: Option<String>,
}

pub struct ConsentEngine {
    client: Client,
    deamon: docker::StoreManager,
    authority: Authority,
}

impl ConsentEngineBuilder {
    pub fn new() -> ConsentEngineBuilder {
        ConsentEngineBuilder {
            address: None,
            port: None,
            deamon: None,
        }
    }

    // Set the address
    pub fn address(&mut self, address: String) -> &mut ConsentEngineBuilder {
        self.address = Some(address);
        self
    }

    // set the port
    pub fn port(&mut self, port: u16) -> &mut ConsentEngineBuilder {
        self.port = Some(port);
        self
    }

    // Set the deamon address + port
    pub fn deamon(&mut self, address: String) -> &mut ConsentEngineBuilder {
        self.deamon = Some(address);
        self
    }

    pub fn build(&self) -> Result<ConsentEngine, String> {
        let address = self.address.clone().ok_or_else(
            || format!("address not present"),
        )?;
        let port = self.port.ok_or_else(|| format!("port not present"))?;

        let deamon_address = self.deamon.clone().ok_or_else(
            || format!("deamon address not present"),
        )?;

        let deamon = docker::StoreManager::new(&deamon_address);

        let authority = Authority::new().unwrap();

        let client = match Client::connect(&address, port) {
            Err(err) => return Err(err.to_string()),
            Ok(c) => c,
        };

        let engine = ConsentEngine {
            client: client,
            deamon: deamon,
            authority: authority,
        };

        let views = engine.get_views().map_err(|e| e.to_string())?;

        let v = views
            .iter()
            .map(|ref x| x.id.clone())
            .collect::<Vec<String>>();

        engine.deamon.start_containers(v)?;
        Ok(engine)
    }
}

impl ConsentEngine {
    pub fn handle_incoming(&self, msg: proto::FromClient) -> Result<proto::ToClient, String> {
        let inner = match msg.msg {
            Some(msg) => msg,
            None => return Err(String::from("empty message")),
        };

        let mut response = proto::ToClient::default();

        info!("Got message {:?}", inner);

        let result = match inner {
            proto::from_client::Msg::Consent(c) => {
                let consent = Consent {
                    id: c.id,
                    purpose: c.purpose,
                    lifetime: Duration::new(c.lifetime, 0),
                    aquired: Utc::now(),
                };
                self.add_consent(&consent)
            }
            proto::from_client::Msg::Intent(i) => {
                let intent = Intent {
                    id: i.id,
                    intent: vec![i.intent],
                };
                self.add_intent(&intent)
            }
            proto::from_client::Msg::User(u) => self.onboard_user(&u.userid),
            proto::from_client::Msg::Certificaterequest(r) => {
                match self.handle_cert_request(r) {
                    Ok(cert) => {
                        response.msg = Some(proto::to_client::Msg::Certificate(cert.clone()))
                    }
                    Err(e) => {
                        error!("{}", e.to_string());
                        return Err(e.to_string());
                    }
                }

                Ok(())
            }
            proto::from_client::Msg::Requestips(_) => {
                match self.get_tenant_ips() {
                    Ok(ips) => {
                        response.msg = Some(proto::to_client::Msg::Ips(
                            proto::Addresses { ip: ips.clone() },
                        ));
                        info!("Successfully responded to requestips");
                    }
                    Err(err) => {
                        error!("{}", err.to_string());
                        return Err(err.to_string());
                    }
                }
                Ok(())
            }
        };

        match result {
            Err(e) => {
                response.msg = Some(proto::to_client::Msg::Error(
                    proto::Error { error: e.to_string() },
                ))
            }
            Ok(_) => response.success = true,
        }
        Ok(response)
    }

    // Retrieve all tenant's ip addresses
    pub fn get_tenant_ips(&self) -> Result<Vec<String>, String> {
        let coll = self.client.db("test").collection("views");
        let cursor = coll.find(None, None).map_err(|e| e.to_string())?;
        let mut ips = Vec::new();

        for entry in cursor {
            if let Ok(item) = entry {
                let view: View = match bson::from_bson(bson::Bson::Document(item)) {
                    Ok(c) => c,
                    Err(err) => {
                        error!("{}", err.to_string());
                        return Err(err.to_string());
                    }
                };
                ips.push(view.ip);
            }
        }

        info!("Came here");
        Ok(ips)
    }

    // add a user consent
    pub fn add_consent(&self, consent: &Consent) -> Result<(), String> {
        let serialized_consent = match bson::to_bson(consent) {
            Ok(res) => res,
            Err(err) => return Err(err.to_string()),
        };

        let consents = self.client.db("test").collection("consents");

        if let bson::Bson::Document(document) = serialized_consent {
            if let Err(e) = consents.insert_one(document, None) {
                error!("{}", e.to_string());
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    // add a user consent
    pub fn remove_user(&self, user_id: &String) -> Result<(), String> {
        let consents = self.client.db("test").collection("consents");

        if let Err(err) = consents.delete_one(doc! { "_id" => user_id }, None) {
            return Err(err.to_string());
        };

        let views = self.client.db("test").collection("views");
        if let Err(err) = views.delete_one(doc! { "_id" => user_id }, None) {
            return Err(err.to_string());
        };

        Ok(())
    }

    pub fn get_consent(&self, id: String) -> Result<Consent, String> {
        let consents = self.client.db("test").collection("consents");

        let consent_doc = match consents.find_one(Some(doc! { "_id" => id }), None) {
            Ok(c) => c.unwrap(),
            Err(err) => return Err(err.to_string()),
        };

        let consent = match bson::from_bson(bson::Bson::Document(consent_doc)) {
            Ok(c) => c,
            Err(err) => return Err(err.to_string()),
        };

        Ok(consent)
    }

    pub fn add_intent(&self, intent: &Intent) -> Result<(), String> {
        let serialized_intent = match bson::to_bson(intent) {
            Ok(res) => res,
            Err(err) => return Err(err.to_string()),
        };

        let intents = self.client.db("test").collection("intents");

        if let bson::Bson::Document(document) = serialized_intent {
            if let Err(e) = intents.insert_one(document, None) {
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    pub fn get_intent(&self, id: &String) -> Result<Intent, String> {
        let intents = self.client.db("test").collection("intents");

        let intent_doc = match intents.find_one(Some(doc! { "_id" => id }), None) {
            Ok(c) => {
                match c {
                    Some(s) => s,
                    None => return Err(String::from("what")),
                }
            }
            Err(err) => return Err(err.to_string()),
        };

        let intent = match bson::from_bson(bson::Bson::Document(intent_doc)) {
            Ok(c) => c,
            Err(err) => return Err(err.to_string()),
        };

        Ok(intent)
    }

    pub fn register_view(&self, view: &View) -> Result<(), String> {
        let serialized_view = match bson::to_bson(view) {
            Ok(res) => res,
            Err(err) => return Err(err.to_string()),
        };

        let views = self.client.db("test").collection("view");

        if let bson::Bson::Document(document) = serialized_view {
            if let Err(e) = views.insert_one(document, None) {
                return Err(e.to_string());
            }
        }

        Ok(())
    }

    // abandon ship boys
    pub fn deboard_user(&self, user_id: &String) -> Result<(), String> {
        self.deamon.remove_container(user_id)?;

        self.remove_user(user_id)?;

        Ok(())
    }

    // retrieve all docker ids
    pub fn get_views(&self) -> Result<Vec<View>, String> {
        let views = self.client.db("test").collection("views");

        let mut vec = Vec::new();

        let cursor = views.find(None, None).map_err(|e| e.to_string())?;
        for entry in cursor {
            if let Ok(item) = entry {
                let view: View = match bson::from_bson(bson::Bson::Document(item)) {
                    Ok(c) => c,
                    Err(err) => return Err(err.to_string()),
                };
                vec.push(view);
            }
        }

        Ok(vec)
    }

    pub fn consent_based_view(&self, id: &String) -> Result<String, String> {
        let coll = self.client.db("test").collection("views");

        let cursor = coll.find(None, None).map_err(|e| e.to_string())?;
        for entry in cursor {
            if let Ok(item) = entry {
                let view: View = match bson::from_bson(bson::Bson::Document(item)) {
                    Ok(c) => c,
                    Err(err) => return Err(err.to_string()),
                };
                if &view.id == id {
                    return Ok(view.ip);
                }
            }
        }
        return Err(String::from("could not find it"));
    }

    pub fn onboard_user(&self, id: &String) -> Result<(), String> {

        match self.deamon.verify_container_id(id) {
            Ok(true) => return Err("user already exists".to_owned()),
            Err(err) => return Err(err.to_string()),
            Ok(false) => (),
        }

        if let Ok(_) = self.consent_based_view(id) {
            error!("Tried to onboard existing user");
            return Err(format!("user already exists"));
        }

        let mut key = BytesMut::new();
        let mut certificate = BytesMut::new();


        self.authority.create_db_certificate(
            &mut key,
            &mut certificate,
        )?;

        let ca = self.authority.get_cert();

        let mut ca_cert = BytesMut::with_capacity(ca.len());

        ca_cert.extend_from_slice(ca.as_slice());

        let mut files = HashMap::new();

        files.insert("certificate.pem", &mut certificate);
        files.insert("keys.pem", &mut key);
        files.insert("CAcert.pem", &mut ca_cert);

        let mut absolute_path = env::current_dir().map_err(|e| e.to_string())?;
        absolute_path.push(id);

        let path = absolute_path.into_os_string().into_string().unwrap();

        self.deamon.new_mountdir(files, &path)?;

        let cert_path = format!(
            "{}/{}:{}:Z",
            path,
            "certificate.pem",
            "/config/certificate.pem"
        );
        let key_path = format!("{}/{}:{}:Z", path, "key.pem", "/config/key.pem");
        let ca_path = format!("{}/{}:{}:Z", path, "CAcert.pem", "/config/CAcert.pem");

        let mut env = Vec::new();

        env.push("PEM_FOLDER=/config");
        env.push("LISTEN_ADDR=3001");
        env.push("DB_ADDR=27017");
        env.push("CA_ADDR=8080");

        let res = self.deamon.new_container(
            &String::from("tenant"),
            id,
            &vec![&cert_path as &str, &key_path as &str, &ca_path as &str],
            env,
        );

        match res {
            Ok(x) => {
                let (id, ip) = x;
                let view = View { id: id, ip: ip };
                if let Err(err) = self.register_view(&view) {
                    error!("{}", err.to_string());
                    return Err(err.to_string());
                };
            }
            Err(err) => {
                error!("{}", err.to_string());
                return Err(err);
            }
        }
        Ok(())
    }

    fn handle_cert_request(&self, req: proto::Certificate) -> Result<proto::Certificate, String> {
        let (intent, cert) = match self.authority.sign_certificate(
            req.request.as_slice(),
            req.intent,
        ) {
            Ok(c) => c,
            Err(e) => return Err(e.to_string()),
        };

        self.add_intent(&intent)?;

        Ok(cert)
    }
}
