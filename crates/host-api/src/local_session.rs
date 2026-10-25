//! Safe Rust wrapper around the thread-confined C session ABI for local tools.

use serde_json::Value;
use std::ffi::CStr;
use std::os::raw::c_char;

fn response(pointer: *mut c_char) -> Result<Value, String> {
    if pointer.is_null() {
        return Err("host API returned no response".into());
    }
    // SAFETY: the C ABI documents an owned NUL-terminated allocation.
    let text = unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map_err(|_| "host API returned invalid UTF-8".to_owned())?
        .to_owned();
    // SAFETY: release the allocation exactly once.
    unsafe { crate::msime_client_string_free(pointer) };
    serde_json::from_str(&text).map_err(|_| "host API returned invalid JSON".into())
}

pub struct LocalSession(u64);

impl LocalSession {
    pub fn create(options: &[u8]) -> Result<Self, String> {
        let value =
            response(unsafe { crate::msime_client_create(options.as_ptr(), options.len()) })?;
        let handle = value
            .get("value")
            .and_then(|value| value.get("session"))
            .and_then(Value::as_u64)
            .ok_or("host API did not return a session handle")?;
        Ok(Self(handle))
    }

    pub fn focus(&self, focused: bool) -> Result<Value, String> {
        response(crate::msime_client_focus(self.0, focused))
    }

    pub fn character(&self, value: u8, shift: bool) -> Result<Value, String> {
        response(crate::msime_client_character(self.0, value, shift))
    }

    pub fn key(&self, key: u32) {
        let _ = crate::msime_client_key(self.0, key);
    }

    pub fn command(&self, command: u32) -> Result<Value, String> {
        response(crate::msime_client_command(self.0, command))
    }

    pub fn select(&self, generation: u64, index: usize) -> Result<Value, String> {
        response(crate::msime_client_select(self.0, generation, index))
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        let _ = response(crate::msime_client_destroy(self.0));
    }
}
