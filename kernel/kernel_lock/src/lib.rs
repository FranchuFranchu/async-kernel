//! Locks that work in different contexts in the kernel

// Needed because of the way lock-api works
#![allow(clippy::declare_interior_mutable_const)]
#![no_std]
#[macro_use]
extern crate log;

extern crate alloc;

pub mod future;
pub mod interrupt;
pub mod shared;
pub mod simple_shared;
pub mod spin;
