use std::error::Error;

use openssl::{ec::{EcKey, EcGroup}, pkey::Private, nid::Nid, ecdsa::EcdsaSig};

pub type PubKey = Vec<u8>;

pub struct Key {
    private_key: EcKey<Private>,
    pub public_key: PubKey,
}

impl Key {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let group = EcGroup::from_curve_name(Nid::SECP256K1)?;
        let key = EcKey::generate(&group)?;
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
            let key = EcKey::private_key_from_der(&data)?;
            let pkey = key.public_key_to_der()?;

            Ok(Self {
                private_key: key,
                public_key: pkey,
            })
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let sign = EcdsaSig::sign(data, &self.private_key)?;
        let sign = sign.to_der()?;

        Ok(sign)
    }

    pub fn validate(data: &[u8], pkey: &[u8], sign: &[u8]) -> Result<bool, Box<dyn Error>> {
        let key = EcKey::public_key_from_der(&pkey)?;
        let verifier = EcdsaSig::from_der(&sign)?;

        Ok(verifier.verify(data, &key)?)
    }
}