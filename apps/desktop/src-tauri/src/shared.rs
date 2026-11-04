//! Host-independent helpers for the desktop shell.

pub(crate) mod account_dto;
pub(crate) mod atomic_file;
pub(crate) mod bounded_body;
pub(crate) mod export_file;
#[cfg(any(target_os = "linux", test))]
pub(crate) mod omarchy_skin;
pub(crate) mod skin_directory;
pub(crate) mod voice;
