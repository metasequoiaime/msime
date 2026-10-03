//! 引擎编译到 WebAssembly，供网页跟打的内置输入法使用。
//!
//! `host` 是纯 Rust 的宿主编排（按键路由、排序、翻页、上文），原生测试直接驱动它；`bindings` 只在 wasm32-unknown-unknown 上编译（和引擎的 `web` 模块同一个条件），用 wasm-bindgen 把它交给浏览器 Worker。

// wasm-bindgen 展开出的胶水代码含 unsafe 块；豁免写在这里而不是退出工作区的 lint 表，和其他 FFI crate 一样。
#![allow(unsafe_code)]

pub mod host;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
mod bindings;
