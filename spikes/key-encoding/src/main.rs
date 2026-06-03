//! RFC-0100 / ADR-0009 de-risk spike T0 — demo binary.

use key_encoding::{
    decode_adjacency_key, decode_path_key, encode_adjacency_key, encode_path_key,
};

fn main() {
    // (full demo binary as written — see real file)
}

fn hex_str(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}
