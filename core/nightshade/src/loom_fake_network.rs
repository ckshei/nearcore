use std::sync::Arc;

use primitives::aggregate_signature::BlsPublicKey;
use primitives::hash::CryptoHash;
use primitives::signature::PublicKey;
use primitives::signer::{BlockSigner, InMemorySigner, TransactionSigner};

use crate::loom_ns_task::Gossip;
use crate::loom_ns_task::NightshadeTask;
use crate::nightshade::BlockProposal;
use primitives::types::AuthorityId;
use std::collections::HashMap;
use loom::sync::Mutex;
use loom::thread;
use loom::fuzz::Builder;

fn spawn_all(num_authorities: usize) {
    let messages_per_node = 1_00i64;
    let mut handles = vec![];
    let gossips: Arc<Mutex<HashMap<AuthorityId, Vec<Gossip>>>> = Arc::new(Mutex::new(HashMap::new()));
    let commitments: Arc<Mutex<HashMap<AuthorityId, BlockProposal>>> = Arc::new(Mutex::new(HashMap::new()));

    let signers: Vec<Arc<InMemorySigner>> =
        (0..num_authorities).map(|_| Arc::new(InMemorySigner::default())).collect();
    let (public_keys, bls_public_keys): (Vec<PublicKey>, Vec<BlsPublicKey>) =
        signers.iter().map(|signer| (signer.public_key(), signer.bls_public_key())).unzip();

    for owner_uid in 0..num_authorities {
        let gossips = gossips.clone();
        let commitments = commitments.clone();
        let block_index = 0;
        let block_hash = CryptoHash::default();
        let public_keys = public_keys.clone();
        let bls_public_keys = bls_public_keys.clone();
        let signer = signers[owner_uid].clone();
        handles.push(thread::spawn(move || {
            let mut task = NightshadeTask::new(
                owner_uid,
                block_index,
                block_hash,
                public_keys,
                bls_public_keys,
                signer,
                gossips,
                commitments,
                messages_per_node,
            );
            task.run();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let mut commitments: Vec<_> = commitments.lock().unwrap().drain().collect();
    println!("COMMITTED {} out of {}", commitments.len(), num_authorities);
    if commitments.len() > 1 {
        let first = commitments.pop().unwrap();
        for c in commitments {
            assert_eq!(c.1, first.1);
        }
    }
}

fn limited_builder() -> Builder {
    let mut builder = Builder::new();
    builder
}

#[cfg(test)]
mod tests {
    use super::{spawn_all, limited_builder};

    #[test]
    fn two_authorities() {
        limited_builder().fuzz(move || {
            spawn_all(2);
        });
    }

//    #[test]
//    fn three_authorities() {
//        spawn_all(3);
//    }
//
//    #[test]
//    fn four_authorities() {
//        spawn_all(4);
//    }
//
//    #[test]
//    fn five_authorities() {
//        spawn_all(5);
//    }
//
//    #[test]
//    fn ten_authorities() {
//        spawn_all(10);
//    }
}
