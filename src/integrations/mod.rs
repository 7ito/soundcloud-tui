#![allow(dead_code)]

pub mod media_controls;

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub mod mpris;
