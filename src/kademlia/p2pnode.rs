use std::collections::{VecDeque, HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::kademlia::p2pstandards::{get_k_bucket_for, NODE_ID_SIZE};

struct NodeTriple {
    node_id: Vec<u8>,
    node_port: u32,
    address: str,
    last_seen: u64,
}

impl NodeTriple {
    pub fn get_node_id(&self) -> &Vec<u8> {
        &self.node_id
    }

    pub fn get_node_port(&self) -> u32 {
        self.node_port.clone()
    }

    pub fn get_address(&self) -> &str {
        &self.address
    }

    pub fn get_last_seen(&self) -> u64 {
        self.last_seen.clone()
    }
}

impl Hash for NodeTriple {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_id.hash(state)
    }
}

struct StoredKeyMetadata {
    key: Vec<u8>,
    owner_node_id: Vec<u8>,
    value: Vec<u8>,
    last_republished: u64,
    last_updated: u64,
}

impl StoredKeyMetadata {
    pub fn key(&self) -> &Vec<u8> {
        &self.key
    }
    pub fn owner_node_id(&self) -> &Vec<u8> {
        &self.owner_node_id
    }
    pub fn value(&self) -> &Vec<u8> {
        &self.value
    }
    pub fn last_republished(&self) -> u64 {
        self.last_republished
    }
    pub fn last_updated(&self) -> u64 {
        self.last_updated
    }
}

impl Hash for StoredKeyMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

struct P2PNode {
    node_id: Vec<u8>,
    k_buckets: Vec<VecDeque<NodeTriple>>,
    node_wait_list: HashMap<u32, Vec<NodeTriple>>,
    seen_messages: HashSet<Vec<u8>>,
    stored_values: HashMap<Vec<u8>, StoredKeyMetadata>,
    published_values: HashMap<Vec<u8>, StoredKeyMetadata>,
}

impl P2PNode {
    pub fn new(node_id: Vec<u8>) -> Self {

        let mut k_bucket = Vec::with_capacity(NODE_ID_SIZE as usize);

        for _i in 0..NODE_ID_SIZE {
            k_bucket.push(VecDeque::new());
        }

        Self {
            node_id,
            k_buckets: k_bucket,
            node_wait_list: HashMap::new(),
            seen_messages: HashSet::new(),
            stored_values: HashMap::new(),
            published_values: HashMap::new()
        }
    }

    pub fn get_node_id(&self) -> &Vec<u8> {
        &self.node_id
    }

    fn get_k_bucket(&self, bucketInd: u32) -> &VecDeque<NodeTriple> {
        assert!(bucketInd < NODE_ID_SIZE);

        &self.k_buckets[bucketInd]
    }


    fn get_k_bucket_mut(&mut self, bucketInd: u32) -> &mut VecDeque<NodeTriple> {
        assert!(bucketInd < NODE_ID_SIZE);

        &mut self.k_buckets[bucketInd]
    }

    fn get_stored_values(&self) -> &HashMap<Vec<u8>, StoredKeyMetadata> {
        &self.stored_values
    }

    fn get_published_values(&self) -> &HashMap<Vec<u8>, StoredKeyMetadata> {
        &self.published_values
    }

    pub fn boostrap(&mut self, boostrap_nodes: Vec<NodeTriple>) {
        for node in boostrap_nodes {
            let k_bucket_for = get_k_bucket_for(node.get_node_id(), self.get_node_id());
            let bucket = self.get_k_bucket_mut(k_bucket_for);

            bucket.push_front(node);

            println!("Populated k bucket " + k_bucket_for + " with node " +
            node.get_node_id());
        }

        //TODO: Perform node lookup operation on our own node id
    }

    fn ping_head_of_bucket(&self) {

    }
}

impl Hash for P2PNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

impl PartialEq<Self> for P2PNode {
    fn eq(&self, other: &Self) -> bool {
        self.node_id.eq(other)
    }
}

impl Eq for P2PNode {}