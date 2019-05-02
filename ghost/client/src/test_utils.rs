use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use actix::actors::mocker::Mocker;
use actix::{Actor, Addr, AsyncContext, Context, Recipient, System};
use futures::{future, Future};

use near_chain::{test_utils::KeyValueRuntime, Block, BlockApproval};
use near_network::types::{FullPeerInfo, PeerChainInfo};
use near_network::{
    NetworkClientMessages, NetworkRequests, NetworkResponses, PeerInfo, PeerManagerActor,
};
use near_store::test_utils::create_test_store;
use primitives::crypto::signer::InMemorySigner;
use primitives::hash::hash;
use primitives::test_utils::init_test_logger;
use primitives::transaction::SignedTransaction;
use primitives::types::MerkleHash;

use crate::{ClientActor, ClientConfig, GetBlock};

pub type NetworkMock = Mocker<PeerManagerActor>;

pub fn setup(
    authorities: Vec<&str>,
    account_id: &str,
    skip_sync_wait: bool,
    recipient: Recipient<NetworkRequests>,
) -> ClientActor {
    let store = create_test_store();
    let runtime = Arc::new(KeyValueRuntime::new_with_authorities(
        store.clone(),
        authorities.into_iter().map(Into::into).collect(),
    ));
    let signer = Arc::new(InMemorySigner::from_seed(account_id, account_id));
    ClientActor::new(
        ClientConfig::test(skip_sync_wait),
        store,
        runtime,
        recipient,
        Some(signer.into()),
    )
    .unwrap()
}

pub fn setup_mock(
    authorities: Vec<&'static str>,
    account_id: &'static str,
    skip_sync_wait: bool,
    mut network_mock: Box<
        FnMut(&NetworkRequests, &mut Context<NetworkMock>, Addr<ClientActor>) -> NetworkResponses,
    >,
) -> Addr<ClientActor> {
    ClientActor::create(move |ctx| {
        let client_addr = ctx.address();
        let pm = NetworkMock::mock(Box::new(move |msg, ctx| {
            let msg = msg.downcast_ref::<NetworkRequests>().unwrap();
            let resp = network_mock(msg, ctx, client_addr.clone());
            Box::new(Some(resp))
        }))
        .start();
        setup(authorities, account_id, skip_sync_wait, pm.recipient())
    })
}
