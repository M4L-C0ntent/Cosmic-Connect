// src/plugins/sms/mod.rs
//! SMS plugin module for KDE Connect.
//!
//! Provides SMS messaging functionality integrated with COSMIC desktop.

// Suppress warnings for code used by binaries (not directly visible to lib crate)
#![allow(dead_code)]
#![allow(unused_imports)]

mod app;
mod dbus;
mod emoji;
mod messages;
mod models;
mod utils;
mod views;

// Re-export the run function for the binary
pub use app::run;