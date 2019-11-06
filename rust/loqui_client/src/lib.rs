#![feature(async_await)]

#[macro_use]
extern crate log;

mod client;
mod config;
// XXX: This is required to be public due to a bug in the rust compiler.
// https://github.com/rust-lang/rust/issues/50865
pub mod connection_handler;
#[cfg(test)]
mod future_utils;
mod waiter;

pub use client::Client;
pub use config::Config;
