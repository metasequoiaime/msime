//! 只用合法设备和当前密钥的元数据规划下载；代次仍须通过信封认证。

use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use msime_client_core::cloud::learning_e2e::{MAX_ENVELOPE_BYTES, MAX_GENERATION};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Deserialize)]
pub(super) struct Metadata {
    pub key_id: Option<String>,
    wrap: Option<Wrap>,
    devices: Vec<Device>,
    pub blobs: Vec<Blob>,
}

#[derive(Deserialize)]
struct Wrap {
    wrap: String,
    approver_device_id: String,
}

#[derive(Deserialize)]
struct Device {
    device_id: String,
    public_key: String,
    approved: bool,
}

#[derive(Deserialize)]
pub(super) struct Blob {
    pub device_id: String,
    pub key_id: String,
    pub revision: u64,
    bytes: usize,
}

impl Metadata {
    pub fn read(body: &[u8], own_id: &str, key: &str) -> Result<Self, RoundError> {
        if body.len() > 1024 * 1024 {
            return Err(RoundError::InvalidMetadata);
        }
        let metadata: Self =
            serde_json::from_slice(body).map_err(|_| RoundError::InvalidMetadata)?;
        if metadata.key_id.as_deref() != Some(key) {
            return Err(RoundError::KeyChanged);
        }
        if metadata.devices.len() > 20 || metadata.blobs.len() > 20 {
            return Err(RoundError::InvalidMetadata);
        }
        let mut devices = BTreeSet::new();
        let mut approved = BTreeSet::new();
        for device in &metadata.devices {
            let public = URL_SAFE_NO_PAD
                .decode(&device.public_key)
                .map_err(|_| RoundError::InvalidMetadata)?;
            if public.len() != 32
                || hex::encode(&Sha256::digest(&public)[..16]) != device.device_id
                || !devices.insert(device.device_id.as_str())
            {
                return Err(RoundError::InvalidMetadata);
            }
            if device.approved {
                approved.insert(device.device_id.as_str());
            }
        }
        let wrap = metadata.wrap.as_ref().ok_or(RoundError::NotReady)?;
        if !approved.contains(own_id)
            || !approved.contains(wrap.approver_device_id.as_str())
            || wrap.wrap.len() != 142
            || !wrap.wrap.starts_with("v1.")
        {
            return Err(RoundError::NotReady);
        }
        let mut blobs = BTreeSet::new();
        for blob in &metadata.blobs {
            if !approved.contains(blob.device_id.as_str())
                || blob.key_id != key
                || !(1..=MAX_GENERATION).contains(&blob.revision)
                || !(84..=MAX_ENVELOPE_BYTES).contains(&blob.bytes)
                || !blobs.insert(blob.device_id.as_str())
            {
                return Err(RoundError::InvalidMetadata);
            }
        }
        Ok(metadata)
    }
}
