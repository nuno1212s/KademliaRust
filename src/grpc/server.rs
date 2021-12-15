use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::kademlia::p2pnode::{NodeTriple, P2PNode, StoredKeyMetadata};
use crate::p2p_server::{Broadcast, BroadcastResponse, FoundNode, FoundValue, Message, MessageResponse, Ping, Store, TargetId};
use crate::p2p_server::p2p_server::P2p;
use crate::p2pstandards::bin_to_hex;

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

    fn notify_node_seen(&self, addr: &Option<SocketAddr>, node_id: Vec<u8>, port: u32) {
        match addr {
            Some(addr) => {
                //Register that we have seen the node
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH).unwrap().as_millis();

                let recv_node_triple = NodeTriple::new(node_id,
                                                       port, addr.ip(),
                                                       current_time,
                                                       self.get_node().get_node_id());

                self.get_node().handle_seen_node(recv_node_triple);
            }
            None => {}
        }
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

        self.notify_node_seen(&possible_addr, ping_req.node_id.clone(),
                              ping_req.requesting_node_port as u32);

        Ok(Response::new(reply))
    }

    async fn store(&self, request: Request<Store>) -> Result<Response<Store>, Status> {
        let possible_addr = request.remote_addr();

        let store_req = request.into_inner();

        self.notify_node_seen(&possible_addr, store_req.requesting_node_id.clone(),
                              store_req.requesting_node_port as u32);

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

        Ok(Response::new(store_req))
    }

    type findNodeStream = ReceiverStream<Result<FoundNode, Status>>;

    async fn find_node(&self, req: Request<TargetId>) -> SendServerStream<Self::findNodeStream> {
        let (tx, rx) = mpsc::channel(4);

        let addr = req.remote_addr();

        let target_req = req.into_inner();

        self.notify_node_seen(&addr, target_req.requesting_node_id.clone(),
                              target_req.request_node_port as u32);

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

    type findValueStream = ReceiverStream<Result<FoundValue, Status>>;

    async fn find_value(&self, req: Request<TargetId>) -> SendServerStream<Self::findValueStream> {
        let (tx, rx) = mpsc::channel(4);

        let addr = req.remote_addr();

        let target_req = req.into_inner();

        self.notify_node_seen(&addr, target_req.requesting_node_id.clone(),
                              target_req.request_node_port as u32);

        let node = self.get_node_ref();

        tokio::spawn(async move {
            let opt_value = node.find_value(&target_req.target_id);

            match opt_value {
                Some(value) => {
                    let reply = FoundValue {
                        value_kind: 0,
                        key: value.key().clone(),
                        value: value.value().clone(),
                        owning_node_id: value.owner_node_id().clone(),
                        last_republished: value.last_republished(),
                        last_updated: value.last_updated(),
                    };

                    tx.send(Ok(reply)).await;
                }
                None => {
                    let closest_nodes = node.find_k_closest_nodes(&target_req.target_id);

                    let time = SystemTime::now().duration_since(UNIX_EPOCH)
                        .unwrap().as_millis();

                    for node in closest_nodes.iter() {
                        let reply = FoundValue {
                            value_kind: 1,
                            key: node.get_node_id().clone(),
                            value: vec![],
                            owning_node_id: value.owner_node_id().clone(),
                            last_republished: time,
                            last_updated: time,
                        };

                        tx.send(Ok(reply)).await;
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn broadcast_message(&self, req: Request<Broadcast>) -> Result<Response<BroadcastResponse>, Status> {
        let addr = req.remote_addr();

        let broadcast = req.into_inner();

        self.notify_node_seen(&addr, broadcast.requesting_node_id.clone(),
                              broadcast.requesting_node_port as u32);

        let mut seen = false;

        if self.get_node().register_seen_msg(broadcast.message_id.clone()) {} else {
            seen = true;
        }

        let reply = BroadcastResponse {
            seen
        };

        Ok(Response::new(reply))
    }

    async fn send_message(&self, req: Request<Message>) -> Result<Response<MessageResponse>,
        Status> {
        let address = req.remote_addr();

        let reply = req.into_inner();

        self.notify_node_seen(&address, reply.sending_node_id.clone(),
                              reply.sending_node_port as u32);

        let resp = MessageResponse {
            response: vec![]
        };

        println!("Received a message from the node {}",
                 bin_to_hex(self.get_node().get_node_id()));

        Ok(Response::new(resp))
    }
}