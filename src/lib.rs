use core::fmt;

use std::{borrow::Cow, convert::TryFrom, rc::Rc};

use futures_util::{
    future::{AbortRegistration, Abortable},
    AsyncBufReadExt, Stream, StreamExt, TryStreamExt,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use cid::{
    multibase::{decode, encode, Base},
    multihash::MultihashGeneric,
    Cid,
};

use reqwest::{
    multipart::{Form, Part},
    Client, Response, Url,
};

pub const DEFAULT_URI: &str = "http://127.0.0.1:5001/api/v0/";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub struct IpfsService {
    client: Client,
    base_url: Rc<Url>,
}

impl IpfsService {
    //TODO new fn for wasm and desktop

    /// Download content from block with this CID.
    pub async fn cat(&self, cid: Cid) -> Result<Vec<u8>> {
        let url = self.base_url.join("cat")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", &cid.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        Ok(bytes.to_vec())
    }

    /// Serialize then add dag node to IPFS. Return a CID.
    pub async fn dag_put<T>(&self, node: &T) -> Result<Cid>
    where
        T: ?Sized + Serialize,
    {
        let data = serde_json::to_vec(node)?;
        let part = Part::bytes(data);
        let form = Form::new().part("object data", part);

        let url = self.base_url.join("dag/put")?;

        let res = self.client.post(url).multipart(form).send().await?;

        let res = match res.json::<DagPutResponse>().await {
            Ok(res) => res,
            Err(e) => return Err(e.into()),
        };

        let cid = Cid::try_from(res.cid.cid_string)?;

        Ok(cid)
    }

    /// Deserialize dag node from IPFS path. Return dag node.
    pub async fn dag_get<U, T>(&self, cid: Cid, path: Option<U>) -> Result<T>
    where
        U: Into<Cow<'static, str>>,
        T: ?Sized + DeserializeOwned,
    {
        let mut origin = cid.to_string();

        if let Some(path) = path {
            origin.push_str(&path.into());
        }

        let url = self.base_url.join("dag/get")?;

        let res = self
            .client
            .post(url)
            .query(&[("arg", &origin)])
            .send()
            .await?;

        let node = res.json::<T>().await?;

        Ok(node)
    }

    /// Resolve IPNS name. Returns CID.
    pub async fn name_resolve(&self, ipns: Cid) -> Result<Cid> {
        let url = self.base_url.join("name/resolve")?;

        let res = self
            .client
            .post(url)
            .query(&[("arg", &ipns.to_string())])
            .send()
            .await?;

        let res = match res.json::<NameResolveResponse>().await {
            Ok(res) => res,
            Err(e) => return Err(e.into()),
        };

        let cid = Cid::try_from(res.path)?;

        Ok(cid)
    }

    pub async fn peer_id(&self) -> Result<Cid> {
        let url = self.base_url.join("id")?;

        let res = self.client.post(url).send().await?;

        let res = match res.json::<IdResponse>().await {
            Ok(res) => res,
            Err(e) => return Err(e.into()),
        };

        let decoded = Base::Base58Btc.decode(res.id)?;
        let multihash = MultihashGeneric::from_bytes(&decoded)?;
        let cid = Cid::new_v1(0x70, multihash);

        Ok(cid)
    }

    pub async fn pubsub_pub<T, D>(&self, topic: T, data: D) -> Result<()>
    where
        T: AsRef<[u8]>,
        D: Into<Cow<'static, [u8]>>,
    {
        let url = self.base_url.join("pubsub/pub")?;

        let topic = encode(Base::Base64Url, topic);

        let part = Part::bytes(data);
        let form = Form::new().part("data", part);

        self.client
            .post(url)
            .query(&[("arg", &topic)])
            .multipart(form)
            .send()
            .await?;

        Ok(())
    }

    pub async fn pubsub_sub_response<T>(&self, topic: T) -> Result<Response>
    where
        T: AsRef<[u8]>,
    {
        let url = self.base_url.join("pubsub/sub")?;

        let topic = encode(Base::Base64Url, topic);

        let response = self
            .client
            .post(url)
            .query(&[("arg", topic)])
            .send()
            .await?;

        Ok(response)
    }

    pub fn pubsub_sub_stream(
        response: Response,
        regis: AbortRegistration,
    ) -> impl Stream<Item = Result<(Cid, Vec<u8>)>> {
        let stream = response.bytes_stream();

        let abortable_stream = Abortable::new(stream, regis);

        //TODO implement from reqwest error for std::io::Error
        let line_stream = abortable_stream
            //.err_into()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
            .into_async_read()
            .lines();

        line_stream.map(|item| match item {
            Ok(line) => {
                if let Ok(response) = serde_json::from_str::<PubsubSubResponse>(&line) {
                    let PubsubSubResponse { from, data } = response;

                    let (_, from) = decode(from)?;
                    let (_, data) = decode(data)?;

                    //Use Peer ID as CID v1 instead of multihash btc58 encoded
                    // https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#string-representation
                    let multihash = MultihashGeneric::from_bytes(&from)?;
                    let cid = Cid::new_v1(0x70, multihash);

                    return Ok((cid, data));
                }

                let ipfs_error = serde_json::from_str::<IPFSError>(&line)?;

                return Err(ipfs_error.into());
            }
            Err(e) => Err(e.into()),
        })
    }
}

#[derive(Deserialize)]
struct PubsubSubResponse {
    pub from: String,
    pub data: String,
}

#[derive(Deserialize)]
struct DagPutResponse {
    #[serde(rename = "Cid")]
    pub cid: CidString,
}

#[derive(Deserialize)]
struct CidString {
    #[serde(rename = "/")]
    pub cid_string: String,
}

#[derive(Deserialize)]
struct NameResolveResponse {
    #[serde(rename = "Path")]
    pub path: String,
}

#[derive(Deserialize)]
struct IdResponse {
    #[serde(rename = "ID")]
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct IPFSError {
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

//#[cfg(target_arch = "wasm32")]
