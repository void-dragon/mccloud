use std::error::Error;

use openssl::{
    // ec::{EcKey, EcGroup},
    pkey::{Private, PKey},
    // nid::Nid,
    // ecdsa::EcdsaSig,
    rsa::Rsa,
    sign::{Signer, Verifier},
    hash::MessageDigest
};

pub type PubKey = Vec<u8>;

#[derive(Clone)]
pub struct Key {
    pub private_key: PKey<Private>,
    pub public_key: PubKey,
}

impl Key {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // let group = EcGroup::from_curve_name(Nid::SECP521R1)?;
        // let group = EcGroup::from_curve_name(Nid::SECP128R2)?;
        // let key = EcKey::generate(&group)?;
        // let key = Rsa::generate(4096)?;
        let key = Rsa::generate(1024)?;
        let key = PKey::from_rsa(key)?;
        let pkey = key.public_key_to_der()?;

        Ok(Self {
            private_key: key,
            public_key: pkey,
        })
    }

    pub fn load(filename: &str) -> Result<Self, Box<dyn Error>> {
        let path = std::path::Path::new(filename);
        if !path.exists() {
            let key = Self::new()?;
            let data = key.private_key.private_key_to_der()?;
            std::fs::write(path, data)?;
            Ok(key)
        }
        else {
            let data = std::fs::read(path)?;
            let key = PKey::private_key_from_der(&data)?;
            // let key = EcKey::private_key_from_der(&data)?;
            let pkey = key.public_key_to_der()?;

            Ok(Self {
                private_key: key,
                public_key: pkey,
            })
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        // let sign = EcdsaSig::sign(data, &self.private_key)?;
        let mut sign = Signer::new(MessageDigest::sha3_512(), &self.private_key)?;
        sign.update(&data)?;
        let sign = sign.sign_to_vec()?;

        Ok(sign)
    }

    pub fn validate(data: &[u8], pkey: &[u8], sign: &[u8]) -> Result<bool, Box<dyn Error>> {
        let key = PKey::public_key_from_der(pkey)?;
        let mut verifier = Verifier::new(MessageDigest::sha3_512(), &key)?;
        verifier.update(data)?;
        // let key = EcKey::public_key_from_der(&pkey)?;
        // let verifier = EcdsaSig::from_der(&sign)?;

        Ok(verifier.verify(sign)?)
    }
}