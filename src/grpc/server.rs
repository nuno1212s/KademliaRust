use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};

use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tonic::codegen::futures_core;
use tonic::codegen::futures_core::Stream;
use tokio_stream::wrappers::ReceiverStream;
use crate::kademlia::p2pnode::{NodeTriple, P2PNode, StoredKeyMetadata};
use crate::p2p_server::p2p_server::{P2p};
use crate::p2p_server::{Ping, Store, FoundNode, TargetId};

#[derive(Debug)]
pub struct MyP2PServer {
    pub node: Arc<P2PNode>,
}

impl MyP2PServer {
    pub fn get_node(&self) -> &P2PNode {
        &self.node
    }

    fn get_node_ref(&self) -> Arc<P2PNode> {
        self.node.clone()
    }
}

type SendServerStream<T> = Result<Response<T>, Status>;

#[tonic::async_trait]
impl P2p for MyP2PServer {
    async fn ping(&self,
                  request: Request<Ping>) -> Result<Response<Ping>, Status> {
        let reply = Ping {
            node_id: self.get_node().get_node_id().clone(),
            requesting_node_port: 50051,
        };

        let possible_addr = request.remote_addr();

        let ping_req = request.into_inner();

        match possible_addr {
            Some(addr) => {
                //Register that we have seen the node
                let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

                let recv_node_triple = NodeTriple::new(ping_req.node_id,
                                                       ping_req.requesting_node_port as u32, addr.ip(),
                                                       current_time,
                                                       self.get_node().get_node_id());

                self.get_node().handle_seen_node(recv_node_triple);
            }
            None => {}
        }

        Ok(Response::new(reply))
    }

    async fn store(&self, request: Request<Store>) -> Result<Response<Store>, Status> {
        let possible_addr = request.remote_addr();

        let store_req = request.into_inner();

        let node_info = self.get_node();

        let curr_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

        let value = StoredKeyMetadata::new(
            store_req.key.clone(),
            store_req.owning_node_id.clone(),
            store_req.value.clone(),
            curr_time,
            curr_time,
        );

        node_info.store_value(value);

        match possible_addr {
            Some(addr) => {
                //Register that we have seen the node
                let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

                let recv_node_triple = NodeTriple::new(store_req.requesting_node_id.clone(),
                                                       store_req.requesting_node_port as u32,
                                                       addr.ip(),
                                                       current_time,
                                                       self.get_node().get_node_id());

                self.get_node().handle_seen_node(recv_node_triple);
            }
            None => {}
        }

        Ok(Response::new(store_req))
    }

    type findNodeStream = ReceiverStream<Result<FoundNode, Status>>;

    async fn find_node(&self, req: Request<TargetId>) -> SendServerStream<Self::findNodeStream> {
        let (tx, rx) = mpsc::channel(4);

        let target_req = req.into_inner();

        let node = self.get_node_ref();

        tokio::spawn(async move {

            let close_nodes =
                node.find_k_closest_nodes(&target_req.target_id);

            for node in close_nodes {
                let node = FoundNode {
                    node_id: (*node.get_node_id()).to_vec(),
                    node_address: node.get_address().to_string(),
                    port: node.get_node_port() as i32,
                    last_seen: node.get_last_seen() as i64,
                };

                tx.send(Ok(node)).await;
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
