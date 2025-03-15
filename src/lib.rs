#![cfg_attr(not(feature = "native-testing"), no_std)]

// When testing natively, use std
#[cfg(feature = "native-testing")]
extern crate std;

mod gnss;
