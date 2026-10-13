//! Deezer catalog plugin: a Hearthsound catalog provider over the Deezer
//! public API.
//!
//! The plugin needs no account and no key, because every endpoint it uses is
//! part of Deezer's public API.
//!
//! The pure logic sits in public modules ([`url`], [`deezer`], [`map`]); the
//! binding-coupled glue that will drive it stays private.
//!
//! # Tests
//!
//! The pure logic is unit-tested on the host target:
//!
//! ```text
//! cargo test
//! ```
//!
//! Rustdoc examples in those modules are illustrative `text` blocks rather than
//! doctests: the crate is a `cdylib` for `wasm32-wasip2`, so there is no host
//! library target for a doctest to link against. Every example appears as an
//! assertion in the unit tests of the module that carries it.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod deezer;
pub mod map;
