use core::fmt;
use std::collections::HashMap;

use cid::Cid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AddResponse {
    #[serde(rename = "Hash")]
    pub hash: String,
}

impl From<AddResponse> for Cid {
    fn from(response: AddResponse) -> Self {
        Cid::try_from(response.hash).unwrap()
    }
}

#[derive(Deserialize)]
pub struct PubsubSubResponse {
    pub from: String,
    pub data: String,
}

#[derive(Deserialize)]
pub struct DagPutResponse {
    #[serde(rename = "Cid")]
    pub cid: CidString,
}

#[derive(Deserialize)]
pub struct CidString {
    #[serde(rename = "/")]
    pub cid_string: String,
}

#[derive(Debug, Deserialize)]
pub struct NamePublishResponse {
    ///IPNS Name
    #[serde(rename = "Name")]
    pub name: String,

    /// CID
    #[serde(rename = "Value")]
    pub value: String,
}

impl From<NameResolveResponse> for Cid {
    fn from(val: NameResolveResponse) -> Self {
        Cid::try_from(val.path).unwrap()
    }
}

#[derive(Deserialize)]
pub struct NameResolveResponse {
    #[serde(rename = "Path")]
    pub path: String,
}

pub type KeyList = HashMap<String, Cid>;

impl From<KeyListResponse> for KeyList {
    fn from(response: KeyListResponse) -> Self {
        HashMap::from_iter(response.keys.into_iter().map(|keypair| {
            let KeyPair { id, name } = keypair;

            let cid = Cid::try_from(id).unwrap();

            (name, cid)
        }))
    }
}

#[derive(Debug, Deserialize)]
pub struct KeyListResponse {
    #[serde(rename = "Keys")]
    pub keys: Vec<KeyPair>,
}

#[derive(Debug, Deserialize)]
pub struct KeyPair {
    #[serde(rename = "Id")]
    pub id: String,

    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Deserialize)]
pub struct IdResponse {
    #[serde(rename = "ID")]
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IPFSError {
    #[serde(rename = "Message")]
    pub message: String,

    #[serde(rename = "Code")]
    pub code: u64,

    #[serde(rename = "Type")]
    pub error_type: String,
}

impl std::error::Error for IPFSError {}

impl fmt::Display for IPFSError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match serde_json::to_string_pretty(&self) {
            Ok(e) => write!(f, "{}", e),
            Err(e) => write!(f, "{}", e),
        }
    }
}

impl From<IPFSError> for std::io::Error {
    fn from(error: IPFSError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, error)
    }
}
