use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use num::BigUint;

use crate::kademlia::p2pstandards::{bin_to_hex, get_k_bucket_for, K, NODE_ID_SIZE};
use crate::operations::operations::{Operation, Operations};
use crate::p2pstandards::distance;

#[derive(Clone, Debug, Eq)]
pub struct NodeTriple {
    node_id: Vec<u8>,
    node_port: u32,
    address: IpAddr,
    last_seen: u128,
    dist_to_current: Vec<u8>,
}

struct SearchNodeTriple {
    node_triple: NodeTriple,
    dist_to_lookup: BigUint,
}

impl SearchNodeTriple {
    fn new(node_triple: NodeTriple, lookup_id: &Vec<u8>) -> Self {
        let distance_to_lookup = distance(node_triple.get_node_id(), lookup_id);

        Self {
            node_triple,
            dist_to_lookup: distance_to_lookup,
        }
    }
}

impl Eq for SearchNodeTriple {}

impl PartialEq<Self> for SearchNodeTriple {
    fn eq(&self, other: &Self) -> bool {
        self.node_triple.get_node_id().eq(other.node_triple.get_node_id())
    }
}

impl Ord for SearchNodeTriple {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dist_to_lookup.cmp(&other.dist_to_lookup)
    }
}

impl PartialOrd<Self> for SearchNodeTriple {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.dist_to_lookup.cmp(&other.dist_to_lookup))
    }
}

impl NodeTriple {
    pub fn new(node_id: Vec<u8>, node_port: u32, address: IpAddr, last_seen: u128,
               local_id: &Vec<u8>) -> Self {
        let distance_ = distance(&node_id, local_id).to_bytes_be();

        Self {
            node_id,
            node_port,
            address,
            last_seen,
            dist_to_current: distance_,
        }
    }

    pub fn get_node_id(&self) -> &Vec<u8> {
        &self.node_id
    }

    pub fn get_node_port(&self) -> u32 {
        self.node_port
    }

    pub fn get_address(&self) -> &IpAddr {
        &self.address
    }

    pub fn get_last_seen(&self) -> u128 {
        self.last_seen
    }


    pub fn set_node_port(&mut self, node_port: u32) {
        self.node_port = node_port;
    }
    pub fn set_address(&mut self, address: IpAddr) {
        self.address = address;
    }
    pub fn set_last_seen(&mut self, last_seen: u128) {
        self.last_seen = last_seen;
    }
}

impl Hash for NodeTriple {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_id.hash(state)
    }
}

impl PartialEq for NodeTriple {
    fn eq(&self, other: &Self) -> bool {
        if self.get_node_id().eq(other.get_node_id()) {
            return true;
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct StoredKeyMetadata {
    key: Vec<u8>,
    owner_node_id: Vec<u8>,
    value: Vec<u8>,
    last_republished: u128,
    last_updated: u128,
}


impl StoredKeyMetadata {
    pub fn new(key: Vec<u8>, owner_id: Vec<u8>, value: Vec<u8>,
               last_republished: u128, last_updated: u128) -> Self {
        Self {
            key,
            owner_node_id: owner_id,
            value,
            last_republished,
            last_updated,
        }
    }

    pub fn key(&self) -> &Vec<u8> {
        &self.key
    }
    pub fn owner_node_id(&self) -> &Vec<u8> {
        &self.owner_node_id
    }
    pub fn value(&self) -> &Vec<u8> {
        &self.value
    }
    pub fn last_republished(&self) -> u128 {
        self.last_republished
    }
    pub fn last_updated(&self) -> u128 {
        self.last_updated
    }
}

impl Hash for StoredKeyMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

#[derive(Debug)]
pub struct P2PNode {
    node_id: Vec<u8>,
    //By locking each k bucket individually, we are able to achieve a much better performance
    k_buckets: Vec<RwLock<VecDeque<NodeTriple>>>,
    node_wait_list: Vec<Mutex<VecDeque<NodeTriple>>>,
    //I don't individually lock these as they have the node_ids as keys
    // and therefore are not distributed across buckets I could maybe do that,
    // but I'm not sure it's actually something worth doing,as it might add some overhead
    // and not really give good performance gains, TODO: will see later
    seen_messages: Mutex<HashSet<Vec<u8>>>,
    stored_values: RwLock<HashMap<Vec<u8>, StoredKeyMetadata>>,
    published_values: Mutex<HashMap<Vec<u8>, StoredKeyMetadata>>,
    //The current active operations on this node
    active_operations: Mutex<Vec<Operations>>,
}

impl P2PNode {
    pub fn new(node_id: Vec<u8>) -> Self {
        let mut k_buckets = Vec::with_capacity(NODE_ID_SIZE as usize);

        let mut node_wait_list = Vec::with_capacity(NODE_ID_SIZE as usize);

        for _i in 0..NODE_ID_SIZE {
            k_buckets.push(RwLock::new(VecDeque::new()));
            node_wait_list.push(Mutex::new(VecDeque::new()));
        }

        Self {
            node_id,
            k_buckets,
            node_wait_list,
            seen_messages: Mutex::new(HashSet::new()),
            stored_values: RwLock::new(HashMap::new()),
            published_values: Mutex::new(HashMap::new()),
            active_operations: Mutex::new(vec![]),
        }
    }

    pub fn get_node_id(&self) -> &Vec<u8> {
        &self.node_id
    }

    fn get_k_bucket(&self, bucket_ind: &u32) -> &RwLock<VecDeque<NodeTriple>> {
        assert!(*bucket_ind < NODE_ID_SIZE);

        &self.k_buckets[*bucket_ind as usize]
    }

    fn get_stored_values(&self) -> &RwLock<HashMap<Vec<u8>, StoredKeyMetadata>> {
        &self.stored_values
    }

    fn get_published_values(&self) -> &Mutex<HashMap<Vec<u8>, StoredKeyMetadata>> {
        &self.published_values
    }

    fn active_operations(&self) -> &Mutex<Vec<Operations>> {
        &self.active_operations
    }

    fn node_wait_list(&self) -> &Vec<Mutex<VecDeque<NodeTriple>>> {
        &self.node_wait_list
    }

    pub fn store_value(&self, value: StoredKeyMetadata) {
        let mut stored_values_lock = self.stored_values.write().unwrap();

        stored_values_lock.insert(value.key().clone(), value);

        ()
    }

    pub fn register_seen_msg(&self, key: Vec<u8>) -> bool {
        let mut seen_msgs = self.seen_messages.lock().unwrap();

        seen_msgs.insert(key)
    }

    pub fn find_value(&self, key: &Vec<u8>) -> Option<StoredKeyMetadata> {
        let stored_values_lock = self.stored_values.read().unwrap();

        match stored_values_lock.get(key) {
            Some(value) => { Option::Some((*value).clone()) }
            None => Option::None
        }
    }

    pub fn boostrap(&self, boostrap_nodes: Vec<NodeTriple>) {
        for node in boostrap_nodes {
            let k_bucket_for = get_k_bucket_for(node.get_node_id(), self.get_node_id());

            let bucket_lock = self.get_k_bucket(&k_bucket_for);

            let mut lck_res = bucket_lock.write().unwrap();

            let bucket = lck_res.deref_mut();

            let nodeid = node.get_node_id().clone();

            bucket.push_front(node);

            println!("Populated k bucket {} with node {}", k_bucket_for,
                     bin_to_hex(&nodeid));
        }

        //TODO: Perform node lookup operation on our own node id
    }

    fn update_sorted_nodes(&self, node_count: u32, k_bucket: u32, sorted_nodes: &mut BTreeSet<SearchNodeTriple>,
                           lookup_id: &Vec<u8>) {
        let k_bucket = self.get_k_bucket(&k_bucket).read().unwrap();

        for node in k_bucket.iter() {
            let search_node = SearchNodeTriple::new((*node).clone(), lookup_id);

            if sorted_nodes.len() < node_count as usize {
                sorted_nodes.insert(search_node);
                continue;
            }

            let last_node_opt = sorted_nodes.last();

            match last_node_opt {
                Some(last_node) => {
                    if search_node.cmp(&last_node) == Ordering::Less {
                        sorted_nodes.pop_last();

                        sorted_nodes.insert(search_node);
                    }
                }

                None => {}
            }
        }
    }

    pub fn find_k_closest_nodes(&self, node_id: &Vec<u8>) -> Vec<NodeTriple> {
        self.find_closest_nodes(K, node_id)
    }

    pub fn find_closest_nodes(&self, node_count: u32, node_id: &Vec<u8>) -> Vec<NodeTriple> {
        let bucket_for_node = get_k_bucket_for(self.get_node_id(), node_id) as i32;

        let mut sorted_nodes = BTreeSet::<SearchNodeTriple>::new();

        //We start looking at the k bucket that the node ID should be contained in,
        //As that is the bucket that contains the nodes that are closest to it
        //Then we start expanding equally to the left and right buckets
        //Until we have filled the node set, or we have gone through all buckets.
        for i_ in 0..NODE_ID_SIZE {
            let i = i_ as i32;

            if bucket_for_node + i < NODE_ID_SIZE as i32 {
                self.update_sorted_nodes(node_count, (bucket_for_node + i) as u32,
                                         &mut sorted_nodes, node_id);
            }

            //i == 0 is already included in the previous if
            if bucket_for_node - i >= 0 && i != 0 {
                self.update_sorted_nodes(node_count, (bucket_for_node - i) as u32, &mut sorted_nodes,
                                         node_id);
            }

            if (bucket_for_node + i > NODE_ID_SIZE as i32 && bucket_for_node - i < 0) || sorted_nodes.len() >= node_count as usize { break; }
        }

        let mut final_vec = Vec::with_capacity(sorted_nodes.len());

        while !sorted_nodes.is_empty() {
            let possible_node = sorted_nodes.pop_first();

            match possible_node {
                Some(node) => {
                    final_vec.push(node.node_triple)
                }
                None => {}
            }
        }

        final_vec
    }

    fn append_waiting_node(&self, seen_node: NodeTriple, k_bucket: &u32) {
        let mut waitlist = self.node_wait_list()[*k_bucket as usize]
            .lock().unwrap();

        waitlist.push_back(seen_node);
    }

    pub fn handle_seen_node(&self, mut seen_node: NodeTriple) {
        let bucket_for_seen_node = get_k_bucket_for(self.get_node_id(),
                                                    seen_node.get_node_id());

        let hex_seen_node_id = bin_to_hex(seen_node.get_node_id());

        println!("Node {} belongs in bucket {}", hex_seen_node_id, bucket_for_seen_node);

        let mut k_bucket =
            self.get_k_bucket(&bucket_for_seen_node).write().unwrap();

        seen_node.set_last_seen(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis());

        let mut already_present = false;

        let mut ind = 0;

        for node in k_bucket.iter() {
            if node.get_node_id().eq(seen_node.get_node_id()) {
                already_present = true;

                break;
            }

            ind += 1;
        }

        k_bucket.remove(ind);

        if already_present {
            k_bucket.push_back(seen_node);

            //We no longer need the lock guard as we aren't going to do any other modifications
            drop(k_bucket);

            println!("We already knew the node {}, moved it to the back", hex_seen_node_id);

            self.pop_latest_seen_node_in_waiting_list(&bucket_for_seen_node);
        } else {
            if k_bucket.len() >= K as usize {

                //We will not be changing the k_bucket any more
                drop(k_bucket);

                //Ping the head of the list, wait for it's response.
                //If it does not respond, remove it and concatenate this node into the last position of the array
                //If it does respond, put it at the tail of the list and ignore this one
                println!("K bucket is full, appending to the wait list and pinging head");

                self.append_waiting_node(seen_node, &bucket_for_seen_node);

                self.ping_head_of_bucket(&bucket_for_seen_node);
            } else {
                k_bucket.push_back(seen_node);

                //I just dropped it here because of the print statement, don't really
                //want to get caught up on OS garbage if we reach large traffic counts
                drop(k_bucket);

                println!("Adding the node {} to the last position of the bucket {}",
                         hex_seen_node_id, bucket_for_seen_node);
            }
        }
    }

    pub fn handle_failed_node_ping(&self, failed_node: &NodeTriple) {
        let bucket_for_failed_node = get_k_bucket_for(self.get_node_id(), failed_node.get_node_id());

        let mut k_bucket = self.get_k_bucket(&bucket_for_failed_node).write().unwrap();

        let mut present = false;
        let mut ind = 0;

        for node in k_bucket.iter() {
            if node.get_node_id().eq(failed_node.get_node_id()) {
                present = true;

                break;
            }

            ind += 1;
        }

        k_bucket.remove(ind);

        if present {
            let latest_node = self.pop_latest_seen_node_in_waiting_list(&bucket_for_failed_node);

            match latest_node {
                Some(node) => {
                    k_bucket.push_back(node);
                }
                None => {}
            }

            //TODO: Remove CRC?
        } else {
            //TODO: Remove CRC?
        }
    }

    pub fn ping_head_of_bucket(&self, bucket: &u32) {
        let bucket_lock = self.get_k_bucket(bucket);

        let bucket = bucket_lock.read().unwrap();

        let option = bucket.front();

        match option {
            Some(node) => {
                //TODO: Ping the node
            }
            None => {}
        }
    }

    fn pop_latest_seen_node_in_waiting_list(&self, k_bucket: &u32) -> Option<NodeTriple> {
        let mut waitlist_lock = self.node_wait_list()[*k_bucket as usize]
            .lock().unwrap();

        waitlist_lock.pop_back()
    }

    fn pop_oldest_seen_node_in_waiting_list(&self, k_bucket: &u32) -> Option<NodeTriple> {
        let mut waitlist_lock = self.node_wait_list()[*k_bucket as usize].lock().unwrap();

        waitlist_lock.pop_front()
    }

    pub fn register_ongoing_operation(&self, operation: Operations) {
        let mut active_ops = self.active_operations.lock().unwrap();

        active_ops.push(operation)
    }

    pub fn mark_operation_closed(&self, operation: Operations) {
        let mut active_ops = self.active_operations.lock().unwrap();

        let pos = active_ops.iter().rposition(|op| {
            if op.eq(&operation) {
                return true;
            }

            false
        });

        match pos {
            Some(index) => {
                active_ops.remove(index);
            }
            None => {}
        }
    }
}

impl Hash for P2PNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_id.hash(state);
    }
}

impl PartialEq<Self> for P2PNode {
    fn eq(&self, other: &Self) -> bool {
        self.get_node_id().eq(other.get_node_id())
    }
}

impl Eq for P2PNode {}