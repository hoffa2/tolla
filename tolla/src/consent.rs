use bson;
use std::ptr;
use mongodb::{Client, ThreadedClient, Error};
use mongodb::db::ThreadedDatabase;
use chrono::prelude::*;
use std::time::{Duration, Instant};
use proto;
use prost::Message;
use register;

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
    pub id: String,
    pub intent: Vec<String>,
}

pub struct ConsentEngine {
    client: Client,
}

impl ConsentEngine {
    pub fn handle_incoming(&self, msg: proto::FromClient) -> Vec<u8> {
        let result = match msg.msg.unwrap() {
            proto::from_client::Msg::Consent(c) => {
                let consent = Consent {
                    id: c.id,
                    purpose: c.purpose,
                    lifetime: Duration::new(c.lifetime, 0),
                    aquired: Utc::now(),
                };
                self.add_consent(&consent)
            },
            proto::from_client::Msg::Intent(i) => {
                let intent = Intent {
                    id: i.id,
                    intent: i.intent,
                };
                self.add_intent(&intent)
            }
        };

        let mut response = proto::ToClient::default();

        match result {
            Err(e) => response.msg =
                Some(proto::to_client::Msg::Error(proto::Error{error: e.to_string()})),
            Ok(_) => response.success = true,
        }

        let mut buf = Vec::new();
        response.encode(&mut buf);
        buf
    }

    pub fn setup(address: &String, port: u16) -> Result<ConsentEngine, Error> {
        let client = match Client::connect(address, port) {
            Err(err) => return Err(err),
            Ok(c) => c,
        };

        Ok(ConsentEngine{
               client: client,
        })
    }

    pub fn add_consent(&self, consent: &Consent) -> Result<(), String> {
        let serialized_consent = match bson::to_bson(consent) {
            Ok(res) => res,
            Err(err) => return Err(err.to_string()),
        };

        let consents = self.client.db("test").collection("consents");

        if let bson::Bson::Document(document) = serialized_consent {
            if let Err(e) = consents.insert_one(document, None) {
                return Err(e.to_string())
            }
        }

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

    pub fn add_intent (&self, intent: &Intent) -> Result<(), String> {
        let serialized_intent = match bson::to_bson(intent) {
            Ok(res) => res,
            Err(err) => return Err(err.to_string()),
        };

        let intents = self.client.db("test").collection("intents");

        if let bson::Bson::Document(document) = serialized_intent {
            if let Err(e) = intents.insert_one(document, None) {
                return Err(e.to_string())
            }
        }

        Ok(())
    }

    pub fn get_intent (&self, id: &String) -> Result<Intent, String> {
        let intents = self.client.db("test").collection("intents");

        let intent_doc = match intents.find_one(Some(doc! { "_id" => id }), None) {
            Ok(c) => c.unwrap(),
            Err(err) => return Err(err.to_string()),
        };

        let intent = match bson::from_bson(bson::Bson::Document(intent_doc)) {
            Ok(c) => c,
            Err(err) => return Err(err.to_string()),
        };

        Ok(intent)
    }

    pub fn consents_from_intent (&self, intent: &Intent) -> Vec<Consent> {
        Vec::new()
    }
}
