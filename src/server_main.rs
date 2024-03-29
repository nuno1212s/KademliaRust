#![feature(map_first_last, fn_traits)]

use std::sync::Arc;
use tonic::{transport::Server};
use grpc::server::MyP2PServer;
use crate::kademlia::p2pnode::P2PNode;

use crate::p2p_server::p2p_server::{P2pServer};
use crate::kademlia::p2pstandards;


pub mod p2p_server {
    tonic::include_proto!("p2pserver");
}

pub mod operations {
    pub mod operations;
    pub mod node_ops;
    pub mod content_ops;
}

mod kademlia;
mod grpc;

// Use the tokio runtime to run our server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();

    let greeter = MyP2PServer {
        node: Arc::new(P2PNode::new(
            p2pstandards::gen_random_id()))
    };

    println!("Starting gRPC Server...");
    Server::builder()
        .add_service(P2pServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}