#![cfg(target_arch = "wasm32")]

/*
Install wasm-pack first then ->

- Command: wasm-pack test --headless --chrome
- Port is random so local IPFS node must accept any
- Chrome must match chromedriver version
*/

use std::assert_eq;

use wasm_bindgen_test::*;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use cid::{multibase::Base, multihash::MultihashGeneric, Cid};
use futures_util::{future::AbortHandle, future::FutureExt, join, StreamExt};
use ipfs_multi_client::IpfsService;

const PEER_ID: &str = "12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC";

#[wasm_bindgen_test]
async fn id() {
    let decoded = Base::Base58Btc.decode(PEER_ID).unwrap();
    let multihash = MultihashGeneric::from_bytes(&decoded).unwrap();
    let cid = Cid::new_v1(0x70, multihash);

    let ipfs = IpfsService::default();

    let id = ipfs.peer_id().await.expect("Getting IPFS ID");

    //console_log!("Peer Id => {:?}", id);

    assert_eq!(id, cid)
}

const TOPIC: &str = "test";
const MSG: &str = "Hello World!";

#[wasm_bindgen_test]
async fn pubsub_roundtrip() {
    let decoded = Base::Base58Btc.decode(PEER_ID).unwrap();
    let multihash = MultihashGeneric::from_bytes(&decoded).unwrap();
    let peer_id = Cid::new_v1(0x70, multihash);

    let ipfs = IpfsService::default();

    let publish = ipfs.pubsub_pub(TOPIC, MSG.as_bytes()).fuse();

    let subscribe = async {
        let ipfs = IpfsService::default();

        let res = ipfs.pubsub_sub_response(TOPIC).await.unwrap();

        let (_, regis) = AbortHandle::new_pair();

        let mut stream = ipfs_multi_client::pubsub_sub_stream(res, regis)
            .take(1)
            .fuse();

        stream.next().await.unwrap()
    };

    let (res, _) = join!(subscribe, publish);

    let (from, data) = res.unwrap();

    //console_log!("From => {:?}", from);
    //console_log!("Data => {:?}", data);

    assert_eq!(from, peer_id);
    assert_eq!(MSG, String::from_utf8(data).unwrap());
}

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct TestBlock {
    data: String,
}

#[wasm_bindgen_test]
async fn dag_roundtrip() {
    let ipfs = IpfsService::default();

    let node = TestBlock {
        data: String::from("This is a test"),
    };

    let cid = ipfs.dag_put(&node).await.unwrap();

    let new_node: TestBlock = ipfs.dag_get(cid, Option::<&str>::None).await.unwrap();

    assert_eq!(node, new_node)
}

const SELF_KEY: &str = "bafzaajaiaejcb3tw3wtri7mxd66jsfeowj627zaktxbssmjykbwyzcqsmm46fbdd";

#[wasm_bindgen_test]
async fn key_listing() {
    let ipfs = IpfsService::default();

    let self_cid = Cid::try_from(SELF_KEY).unwrap();

    let list = ipfs.key_list().await.unwrap();

    assert_eq!(list["self"], self_cid)
}

#[wasm_bindgen_test]
async fn name_publish() {
    let ipfs = IpfsService::default();

    let cid = Cid::try_from("bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci").unwrap();

    let res = ipfs.name_publish(cid, "self").await.unwrap();

    //console_log!("Name Publish => {:?}", res);
}

#[wasm_bindgen_test]
async fn pin_roundtrip() {
    let ipfs = IpfsService::default();

    let cid = Cid::try_from("bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci").unwrap();

    let _ = ipfs.pin_add(cid, false).await;

    let _ = ipfs.pin_rm(cid, false).await;
}
