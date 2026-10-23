//! A fingerprint is an opaque identifier, not a bounded numeric parameter.
//! Accept the original u64 encoding so existing saved work still opens.
use serde::{Deserialize, Deserializer, Serializer};

pub(super) fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("{value:016x}"))
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Legacy(u64),
        Hex(String),
    }
    match Stored::deserialize(deserializer)? {
        Stored::Legacy(value) => Ok(value),
        Stored::Hex(text) if text.len() == 16 && text.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u64::from_str_radix(&text, 16).map_err(serde::de::Error::custom)
        }
        _ => Err(serde::de::Error::custom(
            "Expected a 16-digit preset fingerprint",
        )),
    }
}
