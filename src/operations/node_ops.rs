use std::cell::Ref;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio::time;

use crate::kademlia::p2pnode::NodeTriple;
use crate::operations::operations::{NodeOperationState, Operation, Operations};
use crate::P2PNode;
use crate::p2pstandards::{ALPHA, bin_to_hex, get_k_bucket_for, K, NODE_ID_SIZE, xor};

const TIME_INTERVAL: Duration = Duration::from_millis(1000);

/*
Node lookup operation
Runs a lookup operation on a given ID, as described by the kademlia protocol
*/
pub struct NodeLookupOperation {
    lookup_id: Vec<u8>,
    local_node: Arc<P2PNode>,
    current_ops: Mutex<HashMap<NodeTriple, NodeOperationState>>,
    waiting_resp: Mutex<HashMap<NodeTriple, u128>>,
    called_done: AtomicBool,
    finished: AtomicBool,
    future: Mutex<Option<Arc<JoinHandle<()>>>>,
    on_completed: Box<dyn Fn(Vec<NodeTriple>) -> () + Send + Sync>,
}

/*
Refresh bucket operation

Refreshes a given bucket by performing a NodeLookupOperation on a random ID within that bucket
 */
#[derive(Debug)]
pub struct RefreshBucketOperation {
    k_bucket: u32,
    owning_node: Arc<P2PNode>,
    finished: AtomicBool,
    node_lookup: Mutex<Option<Arc<NodeLookupOperation>>>,
}


impl NodeLookupOperation {
    fn new(lookup_id: Vec<u8>, local_node: Arc<P2PNode>,
           on_completed: fn(Vec<NodeTriple>) -> ()) -> Self {
        Self {
            lookup_id,
            local_node,
            current_ops: Mutex::new(HashMap::new()),
            waiting_resp: Mutex::new(HashMap::new()),
            called_done: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            future: Mutex::new(Option::None),
            on_completed: Box::new(on_completed),
        }
    }


    fn ask(&self) -> bool {
        let mut waiting_resp = self.waiting_resp.lock().unwrap();

        if waiting_resp.len() >= ALPHA as usize {
            //If we have more than alpha messages in transit, do not ask more questions
            return false;
        }

        let mut asked = waiting_resp.len() as u32;

        let nodes_to_ask
            = self.closest_k_nodes_with_state(NodeOperationState::NotAsked);

        if nodes_to_ask.is_empty() && waiting_resp.is_empty() {
            return true;
        }

        let mut current_ops = self.current_ops().lock().unwrap();

        for node in nodes_to_ask.into_iter() {
            current_ops.insert(node.clone(), NodeOperationState::WaitingResponse);

            //TODO: Client perform lookup

            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap().as_millis();

            waiting_resp.insert(node.clone(), current_time);

            asked += 1;

            if asked >= ALPHA {
                break;
            }
        }

        false
    }

    fn closest_k_nodes_with_state(&self, operation_state: NodeOperationState) ->
    Vec<NodeTriple> {
        let current_ops = self.current_ops.lock().unwrap();

        let mut closest_nodes = Vec::with_capacity(current_ops.len());

        for (node, state) in current_ops.iter() {
            if *state == operation_state {
                closest_nodes.push((*node).clone());
            }

            if closest_nodes.len() >= K as usize { break; }
        }

        closest_nodes
    }

    pub fn handle_find_node_failed(&self, node_triple: NodeTriple) {
        let mut waiting_resp = self.waiting_resp().lock().unwrap();

        let mut current_ops = self.current_ops().lock().unwrap();

        waiting_resp.remove(&node_triple);

        drop(waiting_resp);

        self.local_node.handle_failed_node_ping(&node_triple);

        current_ops.insert(node_triple, NodeOperationState::Failed);
    }

    pub fn handle_find_node_success(&self, node_triple: NodeTriple, found_nodes: Vec<NodeTriple>) {
        let mut waiting_resp = self.waiting_resp().lock().unwrap();

        let mut current_ops = self.current_ops().lock().unwrap();

        waiting_resp.remove(&node_triple);

        drop(waiting_resp);

        current_ops.insert(node_triple.clone(), NodeOperationState::Responded);

        self.local_node.handle_seen_node(node_triple);

        for node in found_nodes.into_iter() {
            if !current_ops.contains_key(&node) {
                current_ops.insert(node, NodeOperationState::NotAsked);
            }
        }
    }

    pub fn lookup_id(&self) -> &Vec<u8> {
        &self.lookup_id
    }
    pub fn local_node(&self) -> &Arc<P2PNode> {
        &self.local_node
    }
    pub fn current_ops(&self) -> &Mutex<HashMap<NodeTriple, NodeOperationState>> {
        &self.current_ops
    }
    pub fn waiting_resp(&self) -> &Mutex<HashMap<NodeTriple, u128>> {
        &self.waiting_resp
    }

    pub fn called_done(&self) -> &AtomicBool {
        &self.called_done
    }
    pub fn finished(&self) -> &AtomicBool {
        &self.finished
    }

    fn set_finished(&self, finished: bool) {
        self.finished.store(finished, Ordering::Relaxed);
    }

    fn has_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }
}

impl Operation for NodeLookupOperation {
    fn execute(self: Arc<Self>) {
        let self_res = self.clone();

        let mut opt_future = self.future.lock().unwrap();

        self.local_node().register_ongoing_operation(Operations::NodeLookupOperation(self.clone()));

        let future = tokio::spawn(async move {
            let mut interval = time::interval(TIME_INTERVAL.clone());

            loop {
                if self_res.ask() {
                    if self_res.called_done().compare_exchange(false, true,
                                                               Ordering::Acquire,
                                                               Ordering::Relaxed).is_ok() {
                        let responded_nodes =
                            self_res.closest_k_nodes_with_state(NodeOperationState::Responded);

                        self_res.on_completed.call((responded_nodes, ));

                        let k_bucket = get_k_bucket_for(self_res.lookup_id(),
                                                        self_res.local_node().get_node_id());
                    }

                    self_res.set_finished(true);


                    break;
                }

                interval.tick().await;
            }
        });

        *opt_future = Option::Some(Arc::new(future));
    }

    fn future(self: &Self) -> Option<Arc<JoinHandle<()>>> {
        let future = self.future.lock().unwrap();

        match &*future {
            Some(future_handle) => {
                Option::Some(future_handle.clone())
            }
            None => Option::None
        }
    }

    fn has_finished(self: &Self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    fn identifier(self: &Self) -> &Vec<u8> {
        &self.lookup_id
    }
}

impl PartialEq for NodeLookupOperation {
    fn eq(&self, other: &Self) -> bool {
        if self.lookup_id().eq(other.lookup_id()) {
            return true;
        }

        false
    }
}

impl Eq for NodeLookupOperation {}

impl Debug for NodeLookupOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeLookupOp {{node_id: {}, lookup_id: {}, finished: {:?}}}",
               bin_to_hex(self.local_node().get_node_id()),
               bin_to_hex(self.lookup_id()),
               self.has_finished())
    }
}

impl RefreshBucketOperation {
    fn generate_id_for_bucket(&self) -> Vec<u8> {
        let capacity = (NODE_ID_SIZE / 8) as usize;

        let mut result = Vec::with_capacity(capacity);

        let num_byte_zeros = ((NODE_ID_SIZE - self.k_bucket()) / 8) as usize;
        let num_bit_zeros = (8 - (self.k_bucket() % 8)) as usize;

        for i in 0..num_byte_zeros {
            result[i] = 0;
        }

        let mut byte_set: u8 = 0x01;

        for _i in 0..(8 - num_bit_zeros) {
            byte_set *= 2;
        }

        result[num_byte_zeros] = byte_set;

        for i in num_byte_zeros + 1..capacity {
            result[i] = 0xFF;
        }

        xor(self.owning_node().get_node_id(), &result)
    }

    pub fn k_bucket(&self) -> u32 {
        self.k_bucket
    }

    pub fn owning_node(&self) -> &Arc<P2PNode> {
        &self.owning_node
    }
}

impl Operation for RefreshBucketOperation {
    fn execute(self: Arc<Self>) {
        println!("Refreshing bucket {}", self.k_bucket);

        let node_in_bucket = self.generate_id_for_bucket();

        let node_lookup_op = NodeLookupOperation::new(node_in_bucket,
                                                      self.owning_node.clone(),
                                                      |result| {});

        let arc_node = Arc::new(node_lookup_op);

        let mut node_lookup = self.node_lookup.lock().unwrap();

        *node_lookup = Option::Some(arc_node.clone());

        arc_node.execute();
    }

    fn future(self: &Self) -> Option<Arc<JoinHandle<()>>> {
        let lookup_op = self.node_lookup.lock().unwrap();

        match &*lookup_op {
            Some(lookup_op_r) => {
                lookup_op_r.future()
            }
            None => Option::None
        }
    }

    fn has_finished(self: &Self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    fn identifier(self: &Self) -> &Vec<u8> {
        //TODO: Create an ID for this operation ?
        self.owning_node.get_node_id()
    }
}

impl Eq for RefreshBucketOperation {}

impl PartialEq for RefreshBucketOperation {
    fn eq(&self, other: &Self) -> bool {
        self.identifier().eq(other.identifier())
    }
}