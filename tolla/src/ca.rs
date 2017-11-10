use openssl::rsa::Rsa;
use openssl::pkey::PKey;
use openssl::x509::{X509Builder, X509NameBuilder, X509Extension, X509Req};
use openssl::hash::MessageDigest;
use openssl::asn1::Asn1Time;
use openssl::asn1::Asn1Integer;
use openssl::bn::BigNum;
use openssl::nid;
use uuid::Uuid;
use bytes::{BufMut, BytesMut};
use consent::Intent;
use tolla_proto::proto;

// Structure storing the ca's
// asymetric keypair
pub struct Authority {
    key_pair: PKey,
    root_ctf: Box<Vec<u8>>,
}

impl Authority {
    // Creates a new builder.
    pub fn new() -> Result<Authority, String> {
        let rsa = match Rsa::generate(1024) {
            Ok(kp) => kp,
            Err(e) => return Err(e.to_string()),
        };

        let keypair = match PKey::from_rsa(rsa) {
            Ok(kp) => kp,
            Err(e) => return Err(e.to_string()),
        };

        let mut builder = X509Builder::new().unwrap();
        builder.set_pubkey(&keypair).unwrap();

        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name.append_entry_by_text("C", "NO").unwrap();
        x509_name.append_entry_by_text("ST", "TR").unwrap();
        x509_name.append_entry_by_text("O", "IFI").unwrap();
        x509_name.append_entry_by_text("CN", "localhost").unwrap();
        let x509_name = x509_name.build();

        builder.set_subject_name(&x509_name).unwrap();

        let ca_ext = X509Extension::new(None, None, "basicConstraints", "CA:TRUE").unwrap();

        builder.append_extension(ca_ext).unwrap();

        builder.sign(&keypair, MessageDigest::sha256()).unwrap();

        let x509 = builder.build();

        let x509_pem = x509.to_pem().unwrap();

        Ok(Authority {
            key_pair: keypair,
            root_ctf: Box::new(x509_pem),
        })
    }

    pub fn get_cert(&self) -> Vec<u8> {
        self.root_ctf.clone().to_vec()
    }

    // Create certificate and keypair
    pub fn create_db_certificate(
        &self,
        id: &String,
        keys: &mut BytesMut,
        cert: &mut BytesMut,
    ) -> Result<(), String> {
        let rsa = match Rsa::generate(1024) {
            Ok(kp) => kp,
            Err(e) => return Err(e.to_string()),
        };

        let keypair = match PKey::from_rsa(rsa) {
            Ok(kp) => kp,
            Err(e) => return Err(e.to_string()),
        };

        let mut builder = X509Builder::new().unwrap();
        builder.set_pubkey(&keypair).unwrap();

        builder.set_version(3).map_err(|e| e.to_string())?;
        let bignum = BigNum::from_u32(3).map_err(|e| e.to_string())?;
        let serial_number = bignum.to_asn1_integer().map_err(|e| e.to_string())?;
        builder.set_serial_number(&serial_number).map_err(
            |e| e.to_string(),
        );

        let expiration = Asn1Time::days_from_now(10).unwrap();
        builder.set_not_after(&expiration).unwrap();

        let valid = Asn1Time::days_from_now(0).unwrap();
        builder.set_not_before(&valid).unwrap();

        let mut x509_name = X509NameBuilder::new().map_err(|e| e.to_string())?;
        x509_name.append_entry_by_text("C", "NO").map_err(
            |e| e.to_string(),
        )?;
        x509_name.append_entry_by_text("ST", "TR").map_err(
            |e| e.to_string(),
        )?;
        x509_name.append_entry_by_text("O", "IFI").map_err(
            |e| e.to_string(),
        )?;
        x509_name.append_entry_by_text("CN", "localhost").map_err(
            |e| {
                e.to_string()
            },
        )?;
        let x509_name = x509_name.build();

        builder.set_subject_name(&x509_name).unwrap();

        builder
            .sign(&self.key_pair, MessageDigest::sha256())
            .unwrap();

        let x509 = builder.build();

        let x509_pem = x509.to_pem().map_err(|e| e.to_string())?;
        let keys_pem = keypair.private_key_to_pem().unwrap();

        // make sure that the buffers are all zeroed
        keys.clear();
        cert.clear();

        cert.reserve(x509_pem.len());
        keys.reserve(keys_pem.len());
        cert.put_slice(x509_pem.as_slice());
        keys.put_slice(keys_pem.as_slice());

        Ok(())
    }

    // Create an identity certificate from ceritficate request.
    pub fn sign_certificate(
        &self,
        buf: &[u8],
        intent: String,
    ) -> Result<(Intent, proto::Certificate), String> {

        let mut cert = X509Builder::new().unwrap();
        let req = X509Req::from_pem(buf).unwrap();
        let pubkey = req.public_key().unwrap();

        let expiration = Asn1Time::days_from_now(10).unwrap();
        cert.set_not_after(&expiration).unwrap();

        let valid = Asn1Time::days_from_now(0).unwrap();
        cert.set_not_before(&valid).unwrap();

        cert.set_pubkey(&pubkey).unwrap();
        let mut cn = req.subject_name()
            .entries_by_nid(nid::COMMONNAME)
            .nth(0)
            .ok_or("no common name")?;
        let mut country = req.subject_name()
            .entries_by_nid(nid::COUNTRYNAME)
            .nth(0)
            .ok_or("no contry")?;
        let mut org = req.subject_name()
            .entries_by_nid(nid::ORGANIZATIONNAME)
            .nth(0)
            .ok_or("no org")?;
        let mut state = req.subject_name()
            .entries_by_nid(nid::STATEORPROVINCENAME)
            .nth(0)
            .ok_or("no state")?;

        let mut x509_name = X509NameBuilder::new().map_err(|e| e.to_string())?;

        x509_name.append_entry_by_nid(nid::COMMONNAME, &cn.data().as_utf8().unwrap().to_string());
        x509_name.append_entry_by_nid(nid::COUNTRYNAME, &org.data().as_utf8().unwrap().to_string());
        x509_name.append_entry_by_nid(
            nid::ORGANIZATIONNAME,
            &org.data().as_utf8().unwrap().to_string(),
        );
        x509_name.append_entry_by_nid(
            nid::STATEORPROVINCENAME,
            &state.data().as_utf8().unwrap().to_string(),
        );

        let subject_id = Uuid::new_v4();
        let ext = X509Extension::new_nid(
            None,
            None,
            nid::SUBJECT_KEY_IDENTIFIER,
            &subject_id.simple().to_string(),
        ).map_err(|e| e.to_string())?;

        cert.append_extension(ext).map_err(|e| e.to_string())?;

        let mut s = String::from(intent.clone());
        s.insert_str(0, "otherName:2.5.29.17;UTF8:");

        let ext = X509Extension::new_nid(None, None, nid::SUBJECT_ALT_NAME, &s)
            .map_err(|e| e.to_string())?;

        cert.append_extension(ext).map_err(|e| e.to_string())?;

        cert.sign(&self.key_pair, MessageDigest::sha256()).unwrap();

        let serialized = cert.build().to_pem().map_err(|e| e.to_string())?;

        let intent = Intent {
            id: subject_id.simple().to_string(),
            intent: vec![intent.clone()],
        };

        Ok((
            intent,
            proto::Certificate {
                intent: String::from(""),
                request: serialized,
                root_cert: self.root_ctf.clone().to_vec(),
            },
        ))
    }
}

#[cfg(test)]
mod test {
    use ca;
    use openssl::rsa::Rsa;
    use openssl::pkey::PKey;
    use openssl::x509::X509ReqBuilder;
    use openssl::hash::MessageDigest;
    use openssl::x509::X509;
    use openssl::x509::X509NameBuilder;
    use openssl::nid;

    #[test]
    fn test_create_certificate() {
        let authority = ca::Authority::new();
        assert!(authority.is_ok(), true);
    }

    #[test]
    fn test_cert_request() {
        let authority = ca::Authority::new();
        assert!(authority.is_ok(), true);
        let authority = authority.unwrap();

        let mut req = X509ReqBuilder::new().unwrap();
        let rsa = Rsa::generate(1024).unwrap();

        let keypair = PKey::from_rsa(rsa).unwrap();

        req.set_pubkey(&keypair).unwrap();

        let mut x509_name = X509NameBuilder::new().unwrap();
        x509_name
            .append_entry_by_nid(nid::BASIC_CONSTRAINTS, "ASN1:UTF8String:Purpose")
            .unwrap();
        req.set_subject_name(&x509_name.build()).unwrap();

        req.sign(&keypair, MessageDigest::sha256()).unwrap();

        let req = req.build();

        let pem_raw = req.to_pem().unwrap();

        let res = authority.sign_certificate(&pem_raw).unwrap();

        X509::from_pem(res.1.as_slice()).unwrap();
    }
}
