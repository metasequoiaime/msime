//! Peak heap of one `msime_client_skin_resource` call, with the default `bytes` array and with `encoding: "base64"`.
//!
//! The iOS keyboard extension reads candidate-skin images under the system's per-extension memory limit, so what one call costs is a launch question there, not a speed question. A counting global allocator reports the peak above what was live before the call; the C string handed back is counted, since the host holds it while it parses.
//!
//! `cargo run --release -p msime-host-api --example skin_resource_memory`

// Integration tests and examples are their own crates, so the exemption the
// library root carries does not reach them. Same boundary, same reason: this
// target drives the C ABI directly.
#![allow(unsafe_code)]
use msime_host_api::*;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::CStr;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator and only adds bookkeeping.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same contract as the caller's.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() {
    for mebibytes in [1_usize, 8] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("skins");
        let skin = root.join("sample");
        std::fs::create_dir_all(skin.join("images")).unwrap();
        std::fs::write(
            skin.join("skin.toml"),
            "schema_version = 1\nid = 'sample'\nname = 'Sample'\nversion = '1.0'\n\
             base = 'system'\ntoolbar_stylesheet = 'toolbar.css'\npreview = 'images/preview.png'\n\
             [supports]\nlayouts = ['vertical']\nthemes = ['dark']\n\
             [candidate_window]\nmin_width_dip = 10\n\
             [candidate_window.decoration]\ntop_inset_dip = 1\nwidth_dip = 10\n",
        )
        .unwrap();
        std::fs::write(skin.join("toolbar.css"), ".sample { color: red; }").unwrap();
        // The resource reader checks the extension, not the contents, so any bytes stand in for an image.
        let size = mebibytes * 1024 * 1024 - 1;
        let bytes: Vec<u8> = (0..size)
            .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        std::fs::write(skin.join("images/preview.png"), &bytes).unwrap();
        drop(bytes);
        for encoding in [None, Some("base64")] {
            let mut request = json!({
                "directory": root.to_str().unwrap(),
                "id": "sample",
                "relative": "images/preview.png",
                "kind": "image"
            });
            if let Some(encoding) = encoding {
                request["encoding"] = json!(encoding);
            }
            let request = request.to_string();
            let before = LIVE.load(Ordering::Relaxed);
            PEAK.store(before, Ordering::Relaxed);
            // SAFETY: the request outlives the call.
            let raw = unsafe { msime_client_skin_resource(request.as_ptr(), request.len()) };
            let peak = PEAK.load(Ordering::Relaxed) - before;
            // SAFETY: a fresh owned response, read before it is freed exactly once.
            let response = unsafe { CStr::from_ptr(raw) }.to_bytes();
            let (ok, length) = (response.starts_with(b"{\"ok\":true"), response.len());
            assert!(ok, "{}", String::from_utf8_lossy(response));
            // SAFETY: as above.
            unsafe { msime_client_string_free(raw) };
            println!(
                "{} {size} bytes: ok={ok} response={length} bytes peak={peak} bytes ({:.1}x the file)",
                encoding.unwrap_or("array"),
                peak as f64 / size as f64
            );
        }
    }
}
