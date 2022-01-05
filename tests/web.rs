#![cfg(target_arch = "wasm32")]

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

    console_log!("Peer Id {:?}", id);

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

    console_log!("From {:?}", from);
    console_log!("Data {:?}", data);

    assert_eq!(from, peer_id);
    assert_eq!(MSG, String::from_utf8(data).unwrap());
}
