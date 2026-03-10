#![allow(dead_code)]

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub mod mpris;
