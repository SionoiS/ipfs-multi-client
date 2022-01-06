mod responses;

use std::{borrow::Cow, convert::TryFrom, rc::Rc};

use futures_util::{
    future::{AbortRegistration, Abortable},
    AsyncBufReadExt, Stream, StreamExt, TryStreamExt,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::responses::*;

use cid::{
    multibase::{decode, encode, Base},
    multihash::MultihashGeneric,
    Cid,
};

use reqwest::{
    multipart::{Form, Part},
    Client, Response, Url,
};

use bytes::Bytes;

pub const DEFAULT_URI: &str = "http://127.0.0.1:5001/api/v0/";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub struct IpfsService {
    client: Client,
    base_url: Rc<Url>,
}

impl Default for IpfsService {
    fn default() -> Self {
        let base_url = Url::parse(DEFAULT_URI).expect("Pasrsing URI");
        let base_url = Rc::from(base_url);

        let client = Client::new();

        Self { client, base_url }
    }
}

impl IpfsService {
    pub fn new(url: Url) -> Self {
        let base_url = Rc::from(url);

        let client = Client::new();

        Self { client, base_url }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn add(&self, bytes: Bytes) -> Result<Cid> {
        let url = self.base_url.join("add")?;

        let part = Part::stream(bytes);

        let form = Form::new().part("path", part);

        let response = self
            .client
            .post(url)
            .query(&[("pin", "false")])
            .query(&[("cid-version", "1")])
            .multipart(form)
            .send()
            .await?
            .json::<AddResponse>()
            .await?;

        Ok(response.into())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add<S>(&self, stream: S) -> Result<Cid>
    where
        S: futures_util::stream::TryStream + Send + Sync + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        let url = self.base_url.join("add")?;

        let body = reqwest::Body::wrap_stream(stream);
        let part = Part::stream(body);

        let form = Form::new().part("path", part);

        let response = self
            .client
            .post(url)
            .query(&[("pin", "false")])
            .query(&[("cid-version", "1")])
            .multipart(form)
            .send()
            .await?
            .json::<AddResponse>()
            .await?;

        Ok(response.into())
    }

    /// Download content from block with this CID.
    pub async fn cat(&self, cid: Cid) -> Result<Bytes> {
        let url = self.base_url.join("cat")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", &cid.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        Ok(bytes)
    }

    pub async fn pin_add(&self, cid: Cid, recursive: bool) -> Result<()> {
        let url = self.base_url.join("pin/add")?;

        self.client
            .post(url)
            .query(&[("arg", &cid.to_string())])
            .query(&[("recursive", &recursive.to_string())])
            .send()
            .await?;

        Ok(())
    }

    pub async fn pin_rm(&self, cid: Cid, recursive: bool) -> Result<()> {
        let url = self.base_url.join("pin/rm")?;

        self.client
            .post(url)
            .query(&[("arg", &cid.to_string())])
            .query(&[("recursive", &recursive.to_string())])
            .send()
            .await?;

        Ok(())
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

        let node = self
            .client
            .post(url)
            .query(&[("arg", &origin)])
            .send()
            .await?
            .json::<T>()
            .await?;

        Ok(node)
    }

    /// Returns all IPNS keys on this IPFS node.
    pub async fn key_list(&self) -> Result<KeyList> {
        let url = self.base_url.join("key/list")?;

        let response = self
            .client
            .post(url)
            .query(&[("l", "true"), ("ipns-base", "base32")])
            .send()
            .await?
            .json::<KeyListResponse>()
            .await?;

        Ok(response.into())
    }

    /// Publish new IPNS record.
    pub async fn name_publish<U>(&self, cid: Cid, key: U) -> Result<NamePublishResponse>
    where
        U: Into<Cow<'static, str>>,
    {
        let url = self.base_url.join("name/publish")?;

        let response = self
            .client
            .post(url)
            .query(&[("arg", &cid.to_string())])
            .query(&[("lifetime", "4320h")]) // 6 months
            .query(&[("key", &key.into())])
            .query(&[("ipns-base", "base32")])
            .send()
            .await?
            .json::<NamePublishResponse>()
            .await?;

        Ok(response)
    }

    /// Resolve IPNS name. Returns CID.
    pub async fn name_resolve(&self, ipns: Cid) -> Result<Cid> {
        let url = self.base_url.join("name/resolve")?;

        let response = self
            .client
            .post(url)
            .query(&[("arg", &ipns.to_string())])
            .send()
            .await?
            .json::<NameResolveResponse>()
            .await?;

        Ok(response.into())
    }

    ///Return peer id as cid v1.
    pub async fn peer_id(&self) -> Result<Cid> {
        let url = self.base_url.join("id")?;

        let res = self
            .client
            .post(url)
            .send()
            .await?
            .json::<IdResponse>()
            .await?;

        let decoded = Base::Base58Btc.decode(res.id)?;
        let multihash = MultihashGeneric::from_bytes(&decoded)?;
        let cid = Cid::new_v1(0x70, multihash);

        Ok(cid)
    }

    /// Send data on the specified topic.
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

                //Use Peer ID as CID v1 instead of multihash btc58 encoded
                // https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md#string-representation
                let decoded = Base::Base58Btc.decode(from)?;
                let multihash = MultihashGeneric::from_bytes(&decoded)?;
                let cid = Cid::new_v1(0x70, multihash);

                let (_, data) = decode(data)?;

                return Ok((cid, data));
            }

            let ipfs_error = serde_json::from_str::<IPFSError>(&line)?;

            return Err(ipfs_error.into());
        }
        Err(e) => Err(e.into()),
    })
}
