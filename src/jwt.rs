pub extern crate openssl;

extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate base64;

use std::{path::Path, io::stdin};
use std::fs::File;
use std::io::{Read, Write};
use std::collections::HashMap;

use openssl::sign::Signer;
use openssl::hash::{hash, MessageDigest};
use openssl::pkey::PKey;
use openssl::x509::{X509, X509Req};

use reqwest::{Client, StatusCode};

use crate::{Account, Directory, helper::{gen_key, b64, read_pkey, gen_csr}};

use serde_json::{Value, from_str, to_string, to_value};
use serde::{Serialize, Deserialize};


use crate::error::{Result, ErrorKind};


#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct JwsHeader {
    nonce: String,
    alg: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jwk: Option<Jwk>
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct Jwk {
    e: String,
    kty: String,
    n: String
}

impl Jwk {
    pub fn new(pkey: &PKey<openssl::pkey::Private>) -> Jwk {
        Jwk {
            e: b64(&pkey.rsa().unwrap().e().to_vec()),
            kty: "RSA".to_string(),
            n: b64(&pkey.rsa().unwrap().n().to_vec())
        }
    }
}


#[derive(Serialize, Deserialize, Clone, Default)]
pub(crate) struct Jws <T> where T : Serialize {
    pub(crate) url: String,
    pub(crate) header: JwsHeader,
    pub(crate) payload: T
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct EncodedJws {
    pub(crate) payload: String,
    pub(crate) protected: String,
    pub(crate) signature: String
}

impl <T> Jws <T> where T : Serialize {
    pub(crate) fn serialize(&self, account: &Account) -> Result<String> {
        let nonce = account.directory.get_nonce()?;

        //data.insert("header".to_owned(), to_value(&header)?);

        // payload: b64 of payload
        let payload = to_string(&self.payload)?;

        //println!("Payload: {}", payload);
        let payload64 = if payload == "\"\"" {"".into()} else { b64(&payload.into_bytes()) };

        let protected64 = b64(&to_string(&self.header)?.into_bytes());

        // signature: b64 of hash of signature of {proctected64}.{payload64}
        let signature64 = {
            let mut signer = Signer::new(MessageDigest::sha256(), &account.pkey)?;
            signer
                .update(&format!("{}.{}", protected64, payload64).into_bytes())?;
            b64(&signer.sign_to_vec()?)
        };

        Ok(to_string(&EncodedJws {
            payload: payload64,
            protected: protected64,
            signature: signature64
        })?)    
    }

    pub(crate) fn new(url: &str, account: &Account, payload: T) -> Result<Jws<T>> {    

        let mut header: JwsHeader = JwsHeader::default();
        header.nonce = account.directory.get_nonce()?;
        header.alg = "RS256".into();
        header.url = url.into();
        
        if let Some(kid) = account.pkey_id.clone() {
            header.kid = kid.into();
        } else {
            header.jwk = Some(Jwk::new(&account.pkey));
        }   

        Ok(Jws {
            header,
            payload,
            url: url.into()            
        })
    }
}



#[cfg(test)]
pub mod tests {
    extern crate env_logger;
    use Directory;
use super::*;

    const LETSENCRYPT_STAGING_DIRECTORY_URL: &'static str = "https://acme-staging-v02.api.letsencrypt.org/directory";

    pub fn test_acc() -> Result<Account> {
        Directory::from_url(LETSENCRYPT_STAGING_DIRECTORY_URL)?
            .account_registration()
            .pkey_from_file("tests/private.key").unwrap()
            .register()
    }

    #[test]
    fn test_directory() -> Result<()>{
        Directory::lets_encrypt()?;

        let dir = Directory::from_url(LETSENCRYPT_STAGING_DIRECTORY_URL).unwrap();
        
        assert_eq!(dir.resources.newAccount, "https://acme-staging-v02.api.letsencrypt.org/acme/new-acct");

        assert!(!dir.get_nonce().unwrap().is_empty());

        let pkey = gen_key().unwrap();
        let account = dir.account_registration().register()?;

        assert!(Jws::new(&account.directory.resources.newAccount,&account, "").is_ok());
        Ok(())
    }
}

