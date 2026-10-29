//! 进程级的函数和不需要会话的个人词典函数。它们都可以在任何线程上同时调用。

use std::ffi::{c_char, CString};
use std::path::{Path, PathBuf};

use msime_engine::{
    PersonalDictionaryEntry, PersonalDictionaryKind, RuntimePaths, SchemeSet, SchemeType,
};
use serde::{Deserialize, Serialize};

use crate::abi::{
    field_str, guard, input_str, msime_engine_paths, read_sized, write_owned, Failure, Outcome,
};
use crate::{
    MSIME_ENGINE_ABI_MAJOR, MSIME_ENGINE_ABI_MINOR, MSIME_ENGINE_FLUSH_STOP_THREAD,
    MSIME_ENGINE_MODEL_JAPANESE, MSIME_ENGINE_UNSUPPORTED, MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED,
};

// ---- 版本与错误 ----

/// 运行时这份库的 ABI 版本：`(major << 16) | minor`。主版本与编译时的 `MSIME_ENGINE_ABI_MAJOR` 不同就不要用它；次版本比编译时小时，后加的函数不可用。
#[no_mangle]
pub extern "C" fn msime_engine_abi_version() -> u32 {
    (MSIME_ENGINE_ABI_MAJOR << 16) | MSIME_ENGINE_ABI_MINOR
}

/// 库的发布版本，静态的 NUL 结尾字符串，不要释放。
#[no_mangle]
pub extern "C" fn msime_engine_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// 本线程上一次调用失败的说明（英文，NUL 结尾），没有失败时是空串。有效到本线程下一次调用这个库为止（调它自己除外），不要释放。
#[no_mangle]
pub extern "C" fn msime_engine_last_error() -> *const c_char {
    crate::abi::last_error()
}

/// 释放库交出的字符串（选项以外的 JSON、路径）。NULL 什么也不做。不要用 `free()`，库和调用方可能用不同的分配器。
///
/// # Safety
/// `text` 是 NULL 或这个库交出、还没释放过的字符串。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: 调用方保证它来自 `CString::into_raw`，只释放一次。
        drop(unsafe { CString::from_raw(text) });
    }
}

// ---- 运行时 ----

/// 读四个根目录里的前三个，`dictionaries` 不碰：`msime_engine_prepare_runtime` 在代次准备好之前还没有代次目录，调用方可以不填这一项。
///
/// # Safety
/// `paths` 里前三个字段的字符串指向各自长度的可读字节。
unsafe fn roots(paths: &msime_engine_paths) -> Outcome<[PathBuf; 3]> {
    // SAFETY: 调用方的前提。
    unsafe {
        Ok([
            PathBuf::from(field_str(paths.resources, "paths.resources")?),
            PathBuf::from(field_str(paths.user_data, "paths.user_data")?),
            PathBuf::from(field_str(paths.cache, "paths.cache")?),
        ])
    }
}

/// 读全部四个根目录，个人词典的三个函数用。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读，四个字段的字符串指向各自长度的可读字节。
unsafe fn runtime_paths(paths: *const msime_engine_paths) -> Outcome<RuntimePaths> {
    // SAFETY: 调用方的前提。
    let paths = unsafe { read_sized(paths, "paths") }?;
    // SAFETY: 同上。
    let [resources, user_data, cache] = unsafe { roots(&paths) }?;
    // SAFETY: 同上。
    let dictionaries = unsafe { field_str(paths.dictionaries, "paths.dictionaries") }?;
    Ok(RuntimePaths {
        resources,
        user_data,
        cache,
        dictionaries: PathBuf::from(dictionaries),
    })
}

fn utf8_path(path: &Path) -> Outcome<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Failure::invalid("the path is not UTF-8"))
}

/// 准备运行时：在 `user_data/dictionaries/<content_id>` 原子地准备一个代次（从 `resources` 复制工作词库并回放学习日志），已就绪的代次只补回放。`content_id` 是 1 到 128 个 `[0-9A-Za-z_-]`，随包数据变了就换一个；带点的版本号要先换掉点（`dict-v2_0_39`）。三个根目录必须是绝对路径、互不重叠。`paths.dictionaries` 不读。按全部方案准备：`resources` 里必须有 `msime-pinyin.db` 和 `msime-english.db`；只用其中几个方案时用 `msime_engine_prepare_runtime_for`，只带那几个方案的数据。成功时把代次目录写进 `*dictionaries`（可以是 NULL），用 `msime_engine_string_free` 释放；建会话时它就是 `paths.dictionaries`。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读；`content_id` 指向 `content_id_len` 个可读字节；`dictionaries` 是 NULL 或可写一个指针。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_prepare_runtime(
    paths: *const msime_engine_paths,
    content_id: *const c_char,
    content_id_len: usize,
    dictionaries: *mut *mut c_char,
) -> i32 {
    // SAFETY: 调用方的前提。
    guard(|| unsafe {
        prepare(
            paths,
            content_id,
            content_id_len,
            SchemeSet::ALL,
            dictionaries,
        )
    })
}

/// ABI 1.1 起。同 `msime_engine_prepare_runtime`，只为 `schemes` 里的方案准备：`resources` 只要有这些方案读的词库。`schemes` 的第 n 位是序号为 n 的方案（`1u << MSIME_ENGINE_SCHEME_KOREAN`），要与会话选项的 `enabled_schemes` 相同，不能是 0，不认识的位是 `MSIME_ENGINE_INVALID_ARGUMENT`。有全拼、双拼或五笔时与 `msime_engine_prepare_runtime` 相同，要 `msime-pinyin.db` 和 `msime-english.db`；没有时两者都不要求，`resources` 带了 `msime-english.db` 就照样复制进代次（专用英文的补全与回车学词用它），没带时代次里没有它。日文的 `msime-japanese.dat` 和粤语、注音、笔画的语言库不经过代次，会话从 `resources` 或选项给的路径直接读。
///
/// # Safety
/// 同 `msime_engine_prepare_runtime`。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_prepare_runtime_for(
    paths: *const msime_engine_paths,
    content_id: *const c_char,
    content_id_len: usize,
    schemes: u32,
    dictionaries: *mut *mut c_char,
) -> i32 {
    guard(|| {
        let schemes = scheme_set(schemes)?;
        // SAFETY: 调用方的前提。
        unsafe { prepare(paths, content_id, content_id_len, schemes, dictionaries) }
    })
}

/// `msime_engine_prepare_runtime` 与 `_for` 共用的部分。
///
/// # Safety
/// 同 `msime_engine_prepare_runtime`。
unsafe fn prepare(
    paths: *const msime_engine_paths,
    content_id: *const c_char,
    content_id_len: usize,
    schemes: SchemeSet,
    dictionaries: *mut *mut c_char,
) -> Outcome<()> {
    // SAFETY: 调用方的前提。
    let paths = unsafe { read_sized(paths, "paths") }?;
    // SAFETY: 同上；`dictionaries` 不读。
    let [resources, user_data, cache] = unsafe { roots(&paths) }?;
    // SAFETY: 同上。
    let content_id = unsafe { input_str(content_id, content_id_len, "content_id") }?;
    let prepared = msime_engine::prepare_runtime_paths_for(
        &resources, &user_data, &cache, content_id, schemes,
    )?;
    let directory = utf8_path(&prepared.dictionaries)?;
    // SAFETY: 调用方的前提。
    unsafe { write_owned(dictionaries, directory) };
    Ok(())
}

/// 方案位集合：第 n 位是序号为 n 的方案（`MSIME_ENGINE_SCHEME_*`）。空集合和不认识的位都不合法。
fn scheme_set(schemes: u32) -> Outcome<SchemeSet> {
    if schemes == 0 {
        return Err(Failure::invalid("schemes must name at least one scheme"));
    }
    (0..u32::BITS)
        .filter(|bit| schemes & (1 << bit) != 0)
        .try_fold(SchemeSet::EMPTY, |set, bit| {
            u8::try_from(bit)
                .ok()
                .and_then(SchemeType::from_u8)
                .map(|scheme| set.with(scheme))
                .ok_or_else(|| Failure::invalid(format!("schemes names no scheme at bit {bit}")))
        })
}

/// 立刻写下所有排着的个人上下文学习（平时延迟约 2 秒写）。`flags` 带 `MSIME_ENGINE_FLUSH_STOP_THREAD` 时再停掉后台写线程并等它结束：卸载库、退出进程前用。
#[no_mangle]
pub extern "C" fn msime_engine_flush_learning(flags: u32) -> i32 {
    guard(|| {
        if flags & !MSIME_ENGINE_FLUSH_STOP_THREAD != 0 {
            return Err(Failure::new(
                MSIME_ENGINE_UNSUPPORTED,
                format!("unknown flush flags {flags:#x}"),
            ));
        }
        if flags & MSIME_ENGINE_FLUSH_STOP_THREAD != 0 {
            msime_engine::shutdown();
        } else {
            msime_engine::flush_personal_learning();
        }
        Ok(())
    })
}

/// 内存告警时调：放掉进程级缓存里没有会话在用的日文模型和个人学习模型。正在用的会话不受影响。
#[no_mangle]
pub extern "C" fn msime_engine_release_caches() -> i32 {
    guard(|| {
        msime_engine::release_japanese_dictionary_cache();
        msime_engine::release_idle_personal_models();
        Ok(())
    })
}

/// 关掉所有线程上缓存着的空闲数据库连接。删除或替换数据目录之前调；正被某次调用用着的连接在那次调用结束时关。
#[no_mangle]
pub extern "C" fn msime_engine_close_databases() -> i32 {
    guard(|| {
        msime_engine::close_cached_databases();
        Ok(())
    })
}

/// 把模型的字节直接交给引擎，给没有文件系统或想自己读文件的宿主。`kind` 是 `MSIME_ENGINE_MODEL_*`，`path` 是会话读这个模型的位置（日文是 `resources/msime-japanese.dat` 或选项 `language_dictionaries.japanese`）。`bytes` 为 NULL 且 `bytes_len` 为 0 时撤掉之前交的那份。字节不是合法模型时 `MSIME_ENGINE_INVALID_ARGUMENT`，什么也不改。库复制一份，调用返回后 `bytes` 可以释放。
///
/// # Safety
/// `path` 指向 `path_len` 个可读字节；`bytes` 是 NULL 或指向 `bytes_len` 个可读字节。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_preload_model(
    kind: u32,
    path: *const c_char,
    path_len: usize,
    bytes: *const u8,
    bytes_len: usize,
) -> i32 {
    guard(|| {
        if kind != MSIME_ENGINE_MODEL_JAPANESE {
            return Err(Failure::new(
                MSIME_ENGINE_UNSUPPORTED,
                format!("unknown model kind {kind}"),
            ));
        }
        // SAFETY: 调用方的前提。
        let path = Path::new(unsafe { input_str(path, path_len, "path") }?);
        if !path.is_absolute() {
            return Err(Failure::invalid("the model path must be absolute"));
        }
        if bytes.is_null() {
            if bytes_len != 0 {
                return Err(Failure::null("bytes"));
            }
            msime_engine::unload_japanese_dictionary(path);
            return Ok(());
        }
        // SAFETY: 调用方保证 `bytes` 指向 `bytes_len` 个可读字节。
        let copy: Box<[u8]> = unsafe { std::slice::from_raw_parts(bytes, bytes_len) }.into();
        if !msime_engine::preload_japanese_dictionary(path, copy) {
            return Err(Failure::invalid("the bytes are not a valid model"));
        }
        Ok(())
    })
}

// ---- 个人词典（不需要会话） ----

/// 个人词典的一条，JSON 里的样子。
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryJson {
    kind: KindJson,
    key: String,
    value: String,
    #[serde(default = "default_weight")]
    weight: i64,
}

fn default_weight() -> i64 {
    PersonalDictionaryEntry::DEFAULT_WEIGHT
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum KindJson {
    Pinyin,
    Wubi,
    Wubi98,
    QuickPhrase,
    English,
}

impl KindJson {
    /// 引擎以后加的种类这个版本的 JSON 还没有名字，列表里先略过，和引擎略过它不认识的词库行一样。
    fn of(kind: PersonalDictionaryKind) -> Option<Self> {
        Some(match kind {
            PersonalDictionaryKind::Pinyin => Self::Pinyin,
            PersonalDictionaryKind::Wubi => Self::Wubi,
            PersonalDictionaryKind::Wubi98 => Self::Wubi98,
            PersonalDictionaryKind::QuickPhrase => Self::QuickPhrase,
            PersonalDictionaryKind::English => Self::English,
            _ => return None,
        })
    }
}

impl From<KindJson> for PersonalDictionaryKind {
    fn from(kind: KindJson) -> Self {
        match kind {
            KindJson::Pinyin => Self::Pinyin,
            KindJson::Wubi => Self::Wubi,
            KindJson::Wubi98 => Self::Wubi98,
            KindJson::QuickPhrase => Self::QuickPhrase,
            KindJson::English => Self::English,
        }
    }
}

impl From<EntryJson> for PersonalDictionaryEntry {
    fn from(entry: EntryJson) -> Self {
        Self {
            kind: entry.kind.into(),
            key: entry.key,
            value: entry.value,
            weight: entry.weight,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditJson {
    previous: Option<EntryJson>,
    replacement: Option<EntryJson>,
    #[serde(default)]
    request_id: String,
}

/// 列出个人词典（用户自己加的词，含自动造的词），按种类、编码、词排序，从第 `offset` 条起最多 `limit` 条（`limit` 为 0 时取 100）。`flags` 是 `MSIME_ENGINE_USER_DICT_*`。结果是 JSON：`{"entries":[{"kind":"pinyin","key":"ni'hao","value":"你好","weight":100000}],"has_more":false}`，用 `msime_engine_string_free` 释放。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读；`json` 可写一个指针。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_user_dict_list(
    paths: *const msime_engine_paths,
    offset: u32,
    limit: u32,
    flags: u32,
    json: *mut *mut c_char,
) -> i32 {
    guard(|| {
        if json.is_null() {
            return Err(Failure::null("json"));
        }
        if flags & !MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED != 0 {
            return Err(Failure::new(
                MSIME_ENGINE_UNSUPPORTED,
                format!("unknown list flags {flags:#x}"),
            ));
        }
        // SAFETY: 调用方的前提。
        let paths = unsafe { runtime_paths(paths) }?;
        let limit = if limit == 0 { 100 } else { limit as usize };
        let page = msime_engine::personal_dictionary_entries(
            &paths,
            offset as usize,
            limit,
            flags & MSIME_ENGINE_USER_DICT_INCLUDE_LEARNED != 0,
        )?;
        let entries: Vec<EntryJson> = page
            .entries
            .into_iter()
            .filter_map(|entry| {
                Some(EntryJson {
                    kind: KindJson::of(entry.kind)?,
                    key: entry.key,
                    value: entry.value,
                    weight: entry.weight,
                })
            })
            .collect();
        let text = serde_json::json!({ "entries": entries, "has_more": page.has_more }).to_string();
        // SAFETY: 上面检查过非空。
        unsafe { write_owned(json, text) };
        Ok(())
    })
}

/// 改个人词典：`{"previous": 条目或 null, "replacement": 条目或 null, "request_id": "..."}`，previous 为 null 是新增，replacement 为 null 是删除，都有是替换（previous 必须与列表里的那条完全相同）。`request_id`（1 到 128 个 `[A-Za-z0-9_-]`，可以省略）让同一次编辑重试时不重复生效。还有会话开着（本进程或别的进程，包括 msime 自己的输入法）时返回 `MSIME_ENGINE_BUSY`，什么也不写：先释放这些路径上的会话。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读；`edit_json` 指向 `edit_len` 个可读字节。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_user_dict_edit(
    paths: *const msime_engine_paths,
    edit_json: *const c_char,
    edit_len: usize,
) -> i32 {
    guard(|| {
        // SAFETY: 调用方的前提。
        let paths = unsafe { runtime_paths(paths) }?;
        // SAFETY: 同上。
        let text = unsafe { input_str(edit_json, edit_len, "edit_json") }?;
        let edit: EditJson = serde_json::from_str(text)
            .map_err(|error| Failure::invalid(format!("edit: {error}")))?;
        let previous = edit.previous.map(PersonalDictionaryEntry::from);
        let replacement = edit.replacement.map(PersonalDictionaryEntry::from);
        msime_engine::edit_personal_dictionary(
            &paths,
            previous.as_ref(),
            replacement.as_ref(),
            &edit.request_id,
        )?;
        Ok(())
    })
}

/// 清空学到的一切：换回随包的工作词库、清空学习日志。和 `msime_engine_prepare_runtime` 一样要求 `resources` 里有 `msime-pinyin.db` 和 `msime-english.db`；代次是用 `msime_engine_prepare_runtime_for` 准备的，就用 `msime_engine_reset_learning_for` 传同一个方案集合。还有会话开着时返回 `MSIME_ENGINE_BUSY`，什么也不换。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_reset_learning(paths: *const msime_engine_paths) -> i32 {
    guard(|| {
        // SAFETY: 调用方的前提。
        let paths = unsafe { runtime_paths(paths) }?;
        msime_engine::reset_learned_data(&paths)?;
        Ok(())
    })
}

/// ABI 1.1 起。同 `msime_engine_reset_learning`，只换回 `schemes` 读的词库，规则与 `msime_engine_prepare_runtime_for` 相同：没有全拼、双拼、五笔时不要求 `msime-pinyin.db`，`msime-english.db` 在 `resources` 里有才换回，都没有时只清空日志。`schemes` 的写法同 `msime_engine_prepare_runtime_for`。
///
/// # Safety
/// `paths` 按 `struct_size` 约定可读。
#[no_mangle]
pub unsafe extern "C" fn msime_engine_reset_learning_for(
    paths: *const msime_engine_paths,
    schemes: u32,
) -> i32 {
    guard(|| {
        let schemes = scheme_set(schemes)?;
        // SAFETY: 调用方的前提。
        let paths = unsafe { runtime_paths(paths) }?;
        msime_engine::reset_learned_data_for(&paths, schemes)?;
        Ok(())
    })
}
