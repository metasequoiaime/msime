//! 第三方扩展包的导入：输入当作用户选中的 `.zip` 交给 `plugins::validate`。它与 `plugins::import` 走同一套暂存、解压和清单校验（见 `crates/client-core/src/plugins/import.rs`），只是不装进插件目录，因此覆盖了 zip 目录解析、成员名与路径规则、大小上限和各类清单的解析。
//!
//! 被拒绝是正常结果；要找的是 panic、越界、超时和内存失控。

#![no_main]

use libfuzzer_sys::fuzz_target;
use msime_client_core::plugins;

fuzz_target!(|data: &[u8]| {
    let directory = tempfile::tempdir().expect("temporary directory");
    // `validate` 按扩展名区分文件夹和归档，所以文件必须以 `.zip` 结尾。
    let archive = directory.path().join("pack.zip");
    std::fs::write(&archive, data).expect("write archive");
    let _ = plugins::validate(&archive);
});
