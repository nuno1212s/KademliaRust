use std::fmt::Debug;
use std::sync::Arc;
use tokio::task::JoinHandle;
use crate::kademlia::p2pnode::NodeTriple;
use crate::operations::node_ops::{NodeLookupOperation, RefreshBucketOperation};

pub trait Operation: Sync + Send + Debug {
    fn execute(self: Arc<Self>);

    fn future(self: &Self) -> Option<Arc<JoinHandle<()>>>;

    fn has_finished(self: &Self) -> bool;

    fn identifier(self: &Self) -> &Vec<u8>;
}

pub trait StoreOperation: Operation {
    fn handle_successful_store(self: Arc<Self>, triple: &NodeTriple);

    fn handle_failed_store(self: Arc<Self>, triple: &NodeTriple);
}

#[derive(Eq, PartialEq, Debug)]
pub enum NodeOperationState {
    NotAsked,
    WaitingResponse,
    Responded,
    Failed,
}

#[derive(Eq, PartialEq, Debug)]
pub enum Operations {

    NodeLookupOperation(Arc<NodeLookupOperation>),
    RefreshBucketOperation(Arc<RefreshBucketOperation>)

}