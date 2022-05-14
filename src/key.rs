use std::error::Error;

use k256::ecdsa::{
    SigningKey,
    signature::{Signer, Verifier, Signature},
    VerifyingKey
};


pub type PubKey = Vec<u8>;


///
/// A convenience abstraction over the elliptic curve algorithms provided by OpenSSL.
/// 
#[derive(Clone)]
pub struct Key {
    /// The private key.
    pub private_key: k256::SecretKey,
    /// The bytes of the public key in SEC1 format.
    pub public_key: Vec<u8>,
}

impl Key {
    pub fn new() -> Self {
        let key = k256::SecretKey::random(rand::thread_rng());
        let pkey = key.public_key();
        let encoded: k256::EncodedPoint = pkey.as_affine().into();

        Self {
            private_key: key,
            public_key: encoded.as_bytes().to_vec(),
        }
    }

    pub fn load(filename: &str) -> Result<Self, Box<dyn Error>> {
        let path = std::path::Path::new(filename);
        if !path.exists() {
            let key = Self::new();
            let data = key.private_key.to_sec1_der().unwrap();
            std::fs::write(path, data)?;
            Ok(key)
        }
        else {
            let data = std::fs::read(path)?;
            let key = k256::SecretKey::from_sec1_der(&data)?;
            let pkey = key.public_key();
            let encoded: k256::EncodedPoint = pkey.as_affine().into();

            Ok(Self {
                private_key: key,
                public_key: encoded.as_bytes().into(),
            })
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signer: SigningKey = self.private_key.clone().into();
        let sig: k256::ecdsa::Signature = signer.sign(data);
        let sign = sig.to_vec();

        Ok(sign)
    }

    pub fn validate(data: &[u8], pkey: &[u8], sign: &[u8]) -> Result<(), anyhow::Error> {
        let verifer = VerifyingKey::from_sec1_bytes(pkey)?;
        let sign = k256::ecdsa::Signature::from_bytes(sign)?;
        Ok(verifer.verify(data, &sign)?)
    }
}