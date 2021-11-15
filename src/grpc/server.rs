use tonic::{Request, Response, Status};

use crate::p2p_server::p2p_server::{P2p};
use crate::p2p_server::{Ping};

#[derive(Default)]
pub struct MyP2PServer {}

#[tonic::async_trait]
impl P2p for MyP2PServer {
    async fn ping(&self,
                  request: Request<Ping>) -> Result<Response<Ping>, Status> {
        let reply = Ping {
            node_id: request.into_inner().node_id,
            requesting_node_port: 50051,
        };

        Ok(Response::new(reply))
    }
}

struct Cacher<T, V, K>
    where T: Fn(V) -> K {
    calc: T,
    value: Option<K>,
}

impl<T, V, K> Cacher<T, V, K> where T: Fn(V) -> K {

}