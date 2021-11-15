use tonic::{transport::Server};
use grpc::server::MyP2PServer;

use crate::p2p_server::p2p_server::{P2pServer};

pub mod p2p_server {
    tonic::include_proto!("p2pserver");
}

mod kademlia;
mod grpc;

// Use the tokio runtime to run our server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let greeter = MyP2PServer::default();

    println!("Starting gRPC Server...");
    Server::builder()
        .add_service(P2pServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}