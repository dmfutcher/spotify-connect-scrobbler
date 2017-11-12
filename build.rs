extern crate protobuf_macros;
extern crate rand;

use rand::Rng;
use std::env;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::io::Write;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    protobuf_macros::expand("src/lib.in.rs", &out.join("lib.rs")).unwrap();

    println!("cargo:rerun-if-changed=src/lib.in.rs");
    println!("cargo:rerun-if-changed=src/spirc.rs");
}
