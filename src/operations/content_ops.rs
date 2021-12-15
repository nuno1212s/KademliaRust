use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use crate::kademlia::p2pnode::*;
use crate::operations::operations::*;
use crate::P2PNode;

/*

 * Performs a content lookup operation
 * (Basically a FIND_VALUE operation, which is based on the NodeLookupOperation)
 */
pub struct ContentLookupOperation {
    lookup_id: Vec<u8>,
    own_node: Arc<P2PNode>,
    stored_nodes: Mutex<HashMap<NodeTriple, NodeOperationState>>,
    pending_requests: Mutex<HashMap<NodeTriple, u128>>,
    found_content: AtomicBool,
    finished: AtomicBool,
    result_consumer: Box<dyn Fn(StoredKeyMetadata) -> () + Send + Sync>
}