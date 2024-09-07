// This stuff ripped from booker/inscenerator

use std::fmt::{Debug, Display};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binary16 {
    pub value: [u8; 16],
}

impl Binary16 {
    pub fn from_string(s: &str) -> anyhow::Result<Binary16> {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;

        let value = URL_SAFE_NO_PAD
            .decode(s)
            .context("unable to decode binary 16 chunk")?;
        Ok(Binary16 {
            value: value
                .try_into()
                .map_err(|_e| anyhow::anyhow!("Incorrect key length for binary 16 chunk"))?,
        })
    }

    pub fn from_raw(value: [u8; 16]) -> Binary16 {
        Binary16 { value }
    }

    pub fn from_u64s(low: u64, high: u64) -> Binary16 {
        let mut value: [u8; 16] = [0; 16];
        value[0..8].copy_from_slice(&low.to_le_bytes());
        value[8..16].copy_from_slice(&high.to_le_bytes());
        Binary16 { value }
    }

    pub fn is_zero(&self) -> bool {
        self.value.iter().all(|c| *c == 0)
    }

    pub fn zero() -> Binary16 {
        Binary16 { value: [0; 16] }
    }
}

impl Display for Binary16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        write!(f, "{}", URL_SAFE_NO_PAD.encode(self.value))
    }
}

impl Serialize for Binary16 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Binary16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Binary16::from_string(&s).map_err(serde::de::Error::custom)
    }
}

impl Debug for Binary16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Binary16")
            .field("value", &format!("{}", self))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ContentHash(Binary16);

impl ContentHash {
    pub fn from_string(s: &str) -> anyhow::Result<ContentHash> {
        Binary16::from_string(s).map(ContentHash)
    }

    pub fn from_raw(id: [u8; 16]) -> ContentHash {
        ContentHash(Binary16::from_raw(id))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn zero() -> ContentHash {
        ContentHash(Binary16::zero())
    }

    /// Get the ContentHash for the given input
    pub fn from_content(content: &[u8]) -> ContentHash {
        use sha2::Digest;
        let digest = Sha256::digest(content);
        ContentHash::from_raw(digest.as_slice()[0..16].try_into().unwrap())
    }
}

impl Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for ContentHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Binary16::deserialize(deserializer).map(ContentHash)
    }
}
