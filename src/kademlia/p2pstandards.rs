use std::ops::BitXor;
use num::bigint::BigUint;

//Node ID size, in bits
pub const NODE_ID_SIZE: u32 = 160;
pub const K: u32 = 20;
pub const ALPHA: u32 = 3;
pub const B_K: u32 = 3;

pub fn get_k_bucket_for(node_id_1: &Vec<u8>, node_id_2: &Vec<u8>) -> u32 {
    let dist = distance(node_id_1, node_id_2);

    //This is basically the same as performing the log_2(dist) rounding down
    let bit_len = dist.bits();

    bit_len as u32
}

pub fn distance(id1: &Vec<u8>, id2: &Vec<u8>) -> BigUint {

    let id1_int = BigUint::from_bytes_be(id1);
    let id2_int = BigUint::from_bytes_be(id2);

    id1_int.bitxor(id2_int)
}