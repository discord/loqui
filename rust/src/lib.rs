#![feature(await_macro, async_await, futures_api)]
#![feature(arbitrary_self_types)]
#![feature(pin)]
#![recursion_limit = "128"]

#[macro_use]
extern crate log;

pub mod client;
pub mod protocol;
pub mod server;
