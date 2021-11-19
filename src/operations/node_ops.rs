use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time;
use tokio::task::JoinHandle;

use crate::kademlia::p2pnode::NodeTriple;
use crate::operations::operations::{Consumer, NodeOperationState, Operation};
use crate::P2PNode;
use crate::p2pstandards::{ALPHA, get_k_bucket_for, K};

const TIME_INTERVAL: Duration = Duration::from_millis(1000);

/*
Node lookup operation
Runs a lookup operation on a given ID, as described by the kademlia protocol
*/
#[derive(Debug)]
struct NodeLookupOperation {
    lookup_id: Vec<u8>,
    local_node: Arc<P2PNode>,
    current_ops: Mutex<HashMap<NodeTriple, NodeOperationState>>,
    waiting_resp: Mutex<HashMap<NodeTriple, u128>>,
    called_done: AtomicBool,
    finished: AtomicBool,
    future: Mutex<Option<JoinHandle<()>>>,
    consumer: Box<dyn Consumer<Vec<NodeTriple>>>,
}

impl NodeLookupOperation {
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
}

impl Operation for NodeLookupOperation {

    fn execute(self: Arc<Self>) {
        let self_res = self.clone();

        let future = tokio::spawn(async move {
            let mut interval = time::interval(TIME_INTERVAL.clone());

            loop {
                if self_res.ask() {
                    if self_res.called_done().compare_exchange(false, true,
                                                               Ordering::Acquire,
                                                               Ordering::Relaxed).is_ok() {
                        let responded_nodes =
                            self_res.closest_k_nodes_with_state(NodeOperationState::Responded);

                        self_res.consumer.consume( responded_nodes);

                        let k_bucket = get_k_bucket_for(self_res.lookup_id(),
                                                        self_res.local_node().get_node_id());
                    }

                    self_res.set_finished(true);

                    break;
                }

                interval.tick().await;
            }
        });

        let mut opt_future = self.future.lock().unwrap();

        *opt_future = Option::Some(future);

        self.local_node.register_operation(self.clone());
    }

    fn has_finished(self: Arc<Self>) -> bool {
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