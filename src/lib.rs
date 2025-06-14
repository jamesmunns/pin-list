#![doc = include_str!("../README.md")]

#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod blocking;
