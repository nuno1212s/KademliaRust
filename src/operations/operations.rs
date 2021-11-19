use std::fmt::Debug;
use std::sync::Arc;
use crate::kademlia::p2pnode::NodeTriple;

pub trait Consumer<T>: Sync + Send + Debug {
    fn consume(self: &Self, food: T);
}

pub trait Operation: Sync + Send + Debug {
    fn execute(self: Arc<Self>);

    fn has_finished(self: Arc<Self>) -> bool;

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