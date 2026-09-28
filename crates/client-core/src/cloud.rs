//! Contracts for the account-backed cloud services.
//!
//! Every module here defines data and validation only; the network call and the
//! credentials that authorise it are injected by the platform host, so nothing
//! in this crate ever opens a socket.

pub mod candidates;
pub mod dictionary;
pub mod snapshot_queue;
pub mod snapshot_validation;
pub mod transport;
