use std::error::Error;

use openssl::{
    ec::{EcKey, EcGroup},
    pkey::{Private, PKey},
    nid::Nid,
    sign::{Signer, Verifier},
    hash::MessageDigest, derive::Deriver
};

pub type PubKey = Vec<u8>;

///
/// A convenience abstraction over the elliptic curve algorithms provided by OpenSSL.
/// 
#[derive(Clone)]
pub struct Key {
    /// The private key.
    pub private_key: PKey<Private>,
    /// The bytes of the public key in DER format.
    pub public_key: PubKey,
}

impl Key {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let group = EcGroup::from_curve_name(Nid::SECP256K1)?;
        let key = EcKey::generate(&group)?;
        let key = PKey::from_ec_key(key)?;
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
            let pkey = key.public_key_to_der()?;

            Ok(Self {
                private_key: key,
                public_key: pkey,
            })
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut sign = Signer::new(MessageDigest::sha3_512(), &self.private_key)?;
        sign.update(&data)?;
        let sign = sign.sign_to_vec()?;

        Ok(sign)
    }

    pub fn validate(data: &[u8], pkey: &[u8], sign: &[u8]) -> Result<bool, anyhow::Error> {
        let key = PKey::public_key_from_der(pkey)?;
        let mut verifier = Verifier::new(MessageDigest::sha3_512(), &key)?;
        verifier.update(data)?;

        Ok(verifier.verify(sign)?)
    }

    pub fn shared_secret(&self, pubkey: &PubKey) -> Result<Vec<u8>, anyhow::Error> {
        let pkey = PKey::public_key_from_der(pubkey)?;
        let mut deriver = Deriver::new(&self.private_key)?;
        deriver.set_peer(&pkey)?;

        let buffer = deriver.derive_to_vec()?;

        Ok(buffer)
    }
}