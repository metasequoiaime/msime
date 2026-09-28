//! Host-independent helpers for the desktop shell.
//!
//! `update_body.rs` sits in this directory without a declaration here: it has
//! never had a caller or a module entry in any revision, so wiring it in now
//! would compile code nobody asked for. Left in place rather than deleted
//! because the bound it implements is the one an updater will need.

pub(crate) mod account_dto;
pub(crate) mod atomic_file;
pub(crate) mod export_file;
pub(crate) mod mobile_ai;
pub(crate) mod skin_directory;
pub(crate) mod voice;
