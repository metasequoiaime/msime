//! 浏览器边界（仅 wasm32-unknown-unknown）：wasm-bindgen 导出的几个函数和 `WebEngine`。
//!
//! 输出用 `js_sys::Object`/`Reflect` 拼成普通对象，键名是驼峰，和 TapTapGo 的 `MsimeFrame`（schema `msime-frame-v1`）逐字段对应；按键是打包的 u32 数组（`Key::unpack`）。这里不依赖 serde，也不依赖 web-sys。上下文只能由 Rust 自己写入，这里没有任何设置上下文的出口（D18）。

use std::path::Path;
use std::sync::{Mutex, PoisonError};

use js_sys::{Array, Object, Reflect};
use msime_engine::time::{set_host_clock, HostClock};
use wasm_bindgen::prelude::*;

use crate::host::{Frame, Key, Out, Scheme, WebHost, JAPANESE_DICTIONARY_PATH};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn perf_now() -> f64;
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);
}

/// 最近一次 panic 的信息；wasm-release 是 panic=abort，实例随之失效，Worker 在捕获到 trap 后读它上报。
static LAST_PANIC: Mutex<String> = Mutex::new(String::new());

/// `HostClock` 要的是普通函数指针，wasm-bindgen 的 extern 包一层。
fn steady_ms() -> f64 {
    perf_now()
}

fn wall_ms() -> f64 {
    js_sys::Date::now()
}

#[wasm_bindgen(start)]
pub fn start() {
    set_host_clock(HostClock { steady_ms, wall_ms });
    std::panic::set_hook(Box::new(|info| {
        let message = info.to_string();
        console_error(&message);
        *LAST_PANIC.lock().unwrap_or_else(PoisonError::into_inner) = message;
    }));
}

/// 把词库字节导入内存 VFS 的 `path`（如 `/res/msime-pinyin.db`）；同名库先删掉。
#[wasm_bindgen]
pub fn import_database(path: &str, bytes: &[u8]) -> Result<(), JsError> {
    msime_engine::web::import_database(path, bytes).map_err(|error| JsError::new(&error))
}

/// 把解压后的 `msime-japanese.dat` 交给引擎，日语方案从此用它做整句和词的转换；再次调用替换掉之前那份。字节不是有效的模型时报错，之前那份保持不变。日语引擎要在这之后创建：创建时没有模型的会话只给假名候选。
#[wasm_bindgen]
pub fn import_japanese_dictionary(bytes: Box<[u8]>) -> Result<(), JsError> {
    if msime_engine::preload_japanese_dictionary(Path::new(JAPANESE_DICTIONARY_PATH), bytes) {
        Ok(())
    } else {
        Err(JsError::new(
            "msime-japanese.dat is not a valid MSJPDT1 model",
        ))
    }
}

/// 删除内存 VFS 里的词库；调用前须先释放所有用到它的 `WebEngine`。
#[wasm_bindgen]
pub fn delete_database(path: &str) -> Result<(), JsError> {
    msime_engine::web::delete_database(path).map_err(|error| JsError::new(&error))
}

/// `"msime-engine-wasm <version> <git short sha>"`；构建时没有 `MSIME_GIT_SHA` 则为 `unknown`。
#[wasm_bindgen]
pub fn build_info() -> String {
    format!(
        "msime-engine-wasm {} {}",
        env!("CARGO_PKG_VERSION"),
        option_env!("MSIME_GIT_SHA").unwrap_or("unknown")
    )
}

/// 最近一次 panic 的信息，没有时为空串。
#[wasm_bindgen]
pub fn last_panic() -> String {
    LAST_PANIC
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

#[wasm_bindgen]
pub struct WebEngine {
    host: WebHost,
}

#[wasm_bindgen]
impl WebEngine {
    /// `scheme` 是 `quanpin`、`xiaohe`、`ziranma`、`wubi86`、`japanese` 或 `korean`。拼音方案和五笔的主库须已导入 `/res/msime-pinyin.db`（拼音方案导入网页包的 `msime-pinyin.db`，五笔导入 `msime-wubi86.db`，路径相同）；日语的模型须已经 `import_japanese_dictionary` 交给引擎；韩文不读词库。`model` 是解压后的 `sentence-model.safetensors`，只有拼音方案用它。
    #[wasm_bindgen(constructor)]
    pub fn new(
        scheme: &str,
        page_size: u32,
        model: Option<Box<[u8]>>,
    ) -> Result<WebEngine, JsError> {
        let scheme = Scheme::parse(scheme)
            .ok_or_else(|| JsError::new(&format!("unknown scheme: {scheme}")))?;
        let page_size =
            usize::try_from(page_size).map_err(|error| JsError::new(&error.to_string()))?;
        let host = WebHost::new(scheme, page_size, model.as_deref())
            .map_err(|error| JsError::new(&error))?;
        Ok(WebEngine { host })
    }

    /// 处理一批打包的按键，返回 `MsimeFrame`。
    pub fn keys(&mut self, packed: &[u32]) -> JsValue {
        let keys: Vec<Key> = packed.iter().copied().filter_map(Key::unpack).collect();
        frame_to_js(&self.host.keys(&keys))
    }

    /// 鼠标点击当前页第 `slot` 个候选，返回 `MsimeFrame`。
    pub fn pick(&mut self, slot: u32) -> JsValue {
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        frame_to_js(&self.host.pick(slot))
    }

    pub fn set_model_enabled(&mut self, enabled: bool) {
        self.host.set_model_enabled(enabled);
    }

    /// 页面的退格是否真的删字；出错时停下打开时传 false，空闲退格就不再删上文。
    pub fn set_backspace_deletes(&mut self, deletes: bool) {
        self.host.set_backspace_deletes(deletes);
    }

    /// 新回合：取消组字、清空上下文、重建会话，返回 `MsimeFrame`。
    pub fn reset(&mut self) -> JsValue {
        frame_to_js(&self.host.reset())
    }
}

/// 在新建的普通对象上设置属性不会抛异常，所以忽略 `Reflect::set` 的结果。
fn set(target: &Object, key: &str, value: &JsValue) {
    let _ = Reflect::set(target, &JsValue::from_str(key), value);
}

fn frame_to_js(frame: &Frame) -> JsValue {
    let object = Object::new();
    let out = Array::new();
    for item in &frame.out {
        out.push(&out_to_js(item));
    }
    set(&object, "out", &out);
    set(&object, "composing", &JsValue::from_bool(frame.composing));
    set(&object, "preedit", &JsValue::from_str(&frame.preedit));
    set(&object, "caret", &JsValue::from_f64(f64::from(frame.caret)));
    let page = Array::new();
    for row in &frame.page {
        let entry = Object::new();
        set(&entry, "text", &JsValue::from_str(&row.text));
        set(&entry, "code", &JsValue::from_str(&row.code));
        page.push(&entry);
    }
    set(&object, "page", &page);
    set(
        &object,
        "pageIndex",
        &JsValue::from_f64(f64::from(frame.page_index)),
    );
    set(&object, "hasPrev", &JsValue::from_bool(frame.has_prev));
    set(&object, "hasNext", &JsValue::from_bool(frame.has_next));
    set(
        &object,
        "highlight",
        &JsValue::from_f64(f64::from(frame.highlight)),
    );
    set(&object, "emptyCode", &JsValue::from_bool(frame.empty_code));
    set(&object, "english", &JsValue::from_bool(frame.english));
    set(&object, "modelOn", &JsValue::from_bool(frame.model_on));
    set(&object, "rerankMs", &JsValue::from_f64(frame.rerank_ms));
    object.into()
}

fn out_to_js(item: &Out) -> JsValue {
    let object = Object::new();
    match item {
        Out::Commit { text, seat } => {
            set(&object, "t", &JsValue::from_str("commit"));
            set(&object, "text", &JsValue::from_str(text));
            set(&object, "seat", &JsValue::from_f64(f64::from(*seat)));
        }
        Out::Type(text) => {
            set(&object, "t", &JsValue::from_str("type"));
            set(&object, "text", &JsValue::from_str(text));
        }
        Out::Back { word } => {
            set(&object, "t", &JsValue::from_str("back"));
            set(&object, "word", &JsValue::from_bool(*word));
        }
        Out::Exit => {
            set(&object, "t", &JsValue::from_str("exit"));
        }
    }
    object.into()
}
