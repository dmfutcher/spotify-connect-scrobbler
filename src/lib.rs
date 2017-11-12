#![crate_name = "librespot"]

#![cfg_attr(feature = "cargo-clippy", allow(unused_io_amount))]

// TODO: many items from tokio-core::io have been deprecated in favour of tokio-io
#![allow(deprecated)]

#[macro_use] extern crate log;

#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate futures;
extern crate num_bigint;
extern crate protobuf;
extern crate rand;
extern crate rustfm_scrobble;
extern crate tokio_core;

pub extern crate librespot_core as core;
pub extern crate librespot_protocol as protocol;
pub extern crate librespot_metadata as metadata;

pub mod keymaster;
pub mod scrobbler;

include!(concat!(env!("OUT_DIR"), "/lib.rs"));
