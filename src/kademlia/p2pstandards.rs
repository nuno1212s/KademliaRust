use std::ops::BitXor;
use num::bigint::BigUint;
use std::mem;
use std::fmt::Write;

use rand::Rng;

//Node ID size, in bits
pub const NODE_ID_SIZE: u32 = 160;
pub const K: u32 = 20;
pub const ALPHA: u32 = 3;
pub const B_K: u32 = 3;

pub fn gen_random_id() -> Vec<u8> {

    let byte_count = (NODE_ID_SIZE / (mem::size_of::<u8>() * 8) as u32) as usize;

    let mut vec = Vec::with_capacity(byte_count);

    let mut rng = rand::thread_rng();

    for _i in 0..byte_count {
        vec.push(rng.gen());
    }

    vec
}

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


pub fn bin_to_hex(bin: &Vec<u8>) -> String {

    let mut hex_string = String::with_capacity(bin.len() * 2);

    for byte in bin {
        write!(&mut hex_string, "{:02x}", byte).unwrap()
    }

    hex_string

}