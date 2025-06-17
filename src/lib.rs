#![doc = include_str!("../README.md")]

#![cfg_attr(feature = "nightly", feature(unsize))]
#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod blocking;
