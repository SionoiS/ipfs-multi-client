#[cfg(test)]
mod tests {
    use cid::{multibase::Base, multihash::MultihashGeneric, Cid};
    use futures_util::{future::AbortHandle, future::FutureExt, StreamExt};
    use ipfs_multi_client::IpfsService;

    const PEER_ID: &str = "12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn id() {
        let decoded = Base::Base58Btc.decode(PEER_ID).unwrap();
        let multihash = MultihashGeneric::from_bytes(&decoded).unwrap();
        let cid = Cid::new_v1(0x70, multihash);

        let ipfs = IpfsService::default();

        let id = ipfs.peer_id().await.expect("Getting IPFS ID");

        println!("Peer Id {:?}", id);

        assert_eq!(id, cid)
    }

    const TOPIC: &str = "test";
    const MSG: &str = "Hello World!";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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

        let (res, _) = tokio::join!(subscribe, publish);

        let (from, data) = res.unwrap();

        println!("From {:?}", from);
        println!("Data {:?}", data);

        assert_eq!(from, peer_id);
        assert_eq!(MSG, String::from_utf8(data).unwrap());
    }
}
