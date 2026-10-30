#!/usr/bin/env python3
"""Android 接「我的词条」网盘同步（`msime_client_dictionary_sync`）时与 Rust 之间的几处契约。

1. 错误码：`DictionarySyncPolicy.CODES`（platforms/android/java/app/msime/android/policy/DictionarySyncPolicy.java）照抄了 `crates/client-core/src/cloud/dictionary_sync/sync.rs` 的 `SyncError::code` 和 `crates/client-services/src/dictionary_webdav.rs` 的 `DictionarySyncError::code`。Rust 加了码而 Java 没跟上，Android 上那个码就只能给出通用的「没有完成」；这里核对两份清单完全相同，并且每个码都写在头文件的说明里。
2. 请求上限：JNI 的 `kDictionarySyncRequestLimit`、`NativeClient.DICTIONARY_SYNC_REQUEST_LIMIT` 与 host-api 的 `MAX_REQUEST_BYTES` 都是 2 MiB，JNI 在复制数组之前就按它拒绝。
3. 目录：两个进程给 Rust 的都是 `noBackupFilesDir/dictionary-sync`，与 docs/dictionary-webdav-sync.md 写的相同；目录不同会让同一台设备登记成两台。
4. 键盘进程（`ImeDictionarySync`）只用 `local_change` 和 `periodic`，跑之前先看本机有没有设备编号（`imeMayRun`），从不调用会登记或换钥的动作（`devices`、`approve`、`reject`、`remove`、`reset`、`restart`、`probe`），而且在自己的工作线程上跑、不排在建会话用的 `preferencesWorker` 后面。设置页打开时用 `opened`，「立即同步」用 `manual`。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SYNC_RS = ROOT / "crates/client-core/src/cloud/dictionary_sync/sync.rs"
SERVICES_RS = ROOT / "crates/client-services/src/dictionary_webdav.rs"
HOST_RS = ROOT / "crates/host-api/src/dictionary_webdav.rs"
HEADER = ROOT / "crates/host-api/include/msime_client.h"
ANDROID = ROOT / "platforms/android"
POLICY = ANDROID / "java/app/msime/android/policy/DictionarySyncPolicy.java"
BRIDGE = ANDROID / "java/app/msime/android/dictionary/DictionaryWebDavSync.java"
IME = ANDROID / "java/app/msime/android/core/ImeDictionarySync.java"
SERVICE = ANDROID / "java/app/msime/android/core/MSIMEInputService.java"
PAGE = ANDROID / "java/app/msime/android/home/DictionarySyncPage.java"
NATIVE = ANDROID / "java/app/msime/android/core/NativeClient.java"
JNI = ANDROID / "native/client_jni.cpp"
DOC = ROOT / "docs/dictionary-webdav-sync.md"


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def block(text: str, pattern: str, label: str, errors: list) -> str:
    match = re.search(pattern, text, re.DOTALL)
    if match is None:
        errors.append(f"没有读到 {label}")
        return ""
    return match.group(1)


def main() -> int:
    errors: list = []

    # 1. 错误码
    sync_codes = block(read(SYNC_RS), r"pub fn code\(self\) -> &'static str \{(.*?)\n    \}", "SyncError::code", errors)
    service_codes = block(read(SERVICES_RS), r"pub fn code\(&self\) -> &str \{(.*?)\n    \}", "DictionarySyncError::code", errors)
    rust = set(re.findall(r'"([a-z_0-9]+)"', sync_codes + service_codes))
    policy = read(POLICY)
    constants = dict(re.findall(r'static final String (CODE_[A-Z_]+) = "([a-z_0-9]+)";', policy))
    listed = block(policy, r"CODES = Arrays\.asList\((.*?)\);", "DictionarySyncPolicy.CODES", errors)
    java = set()
    for item in (part.strip() for part in listed.split(",")):
        if not item:
            continue
        if item.startswith('"'):
            java.add(item.strip('"'))
        elif item in constants:
            java.add(constants[item])
        else:
            errors.append(f"CODES 里认不出的项: {item}")
    if not rust:
        errors.append("Rust 一侧没有读到错误码")
    for label, missing in (("Rust 有、Java 没有", rust - java), ("Java 有、Rust 没有", java - rust)):
        if missing:
            errors.append(f"词库同步错误码 {label}: {', '.join(sorted(missing))}")
    header = read(HEADER)
    entry = next((line for line in header.splitlines() if "invalid_dictionary_sync" in line and "/*" in line), "")
    for code in sorted(rust):
        if code not in entry:
            errors.append(f"msime_client.h 的 msime_client_dictionary_sync 说明里没有 {code}")
    for code in sorted(java):
        if f"case CODE_{next((k[5:] for k, v in constants.items() if v == code), '')} ->" not in policy:
            errors.append(f"DictionarySyncPolicy.failureMessage 没有单独处理 {code}")

    # 2. 请求上限
    host = read(HOST_RS)
    if "pub(crate) const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;" not in host:
        errors.append("host-api 的 MAX_REQUEST_BYTES 不再是 2 MiB，Android 的上限要跟着改")
    jni = read(JNI)
    if "constexpr jsize kDictionarySyncRequestLimit = 2 * 1024 * 1024;" not in jni:
        errors.append("client_jni.cpp 缺少 2 MiB 的 kDictionarySyncRequestLimit")
    start = jni.find("Java_app_msime_android_NativeClient_dictionarySyncRaw")
    body = jni[start:jni.find("\n}", start)] if start >= 0 else ""
    if "bounded_request(env, request, kDictionarySyncRequestLimit" not in body or "msime_client_dictionary_sync" not in body:
        errors.append("dictionarySyncRaw 必须在 JNI 复制数组之前按 kDictionarySyncRequestLimit 拒绝，并调用 msime_client_dictionary_sync")
    if "DICTIONARY_SYNC_REQUEST_LIMIT = 2 * 1024 * 1024;" not in read(NATIVE):
        errors.append("NativeClient.DICTIONARY_SYNC_REQUEST_LIMIT 不是 2 MiB")

    # 3. 目录
    if 'DIRECTORY = "dictionary-sync";' not in policy:
        errors.append("DictionarySyncPolicy.DIRECTORY 不是 dictionary-sync")
    bridge = strip_comments(read(BRIDGE))
    if "new File(context.getNoBackupFilesDir(), DictionarySyncPolicy.DIRECTORY)" not in bridge:
        errors.append("DictionaryWebDavSync.directory 必须是 noBackupFilesDir 下的 DictionarySyncPolicy.DIRECTORY")
    if "noBackupFilesDir/dictionary-sync" not in read(DOC):
        errors.append("docs/dictionary-webdav-sync.md 没有写 Android 的目录 noBackupFilesDir/dictionary-sync")

    # 4. 触发与身份
    ime = strip_comments(read(IME))
    for forbidden in ("TRIGGER_OPENED", "TRIGGER_MANUAL", ".devices(", ".approve(", ".reject(", ".remove(", ".reset(", ".restart(", ".probe("):
        if forbidden in ime:
            errors.append(f"ImeDictionarySync 不能用 {forbidden}：键盘进程只记录改动和定时同步，不登记、不换钥")
    run = ime.find("sync.run(")
    gate = ime.find("DictionarySyncPolicy.imeMayRun(")
    if run < 0 or gate < 0 or gate > run:
        errors.append("ImeDictionarySync 必须先用 imeMayRun 看本机有没有设备编号再 run")
    if "Executors.newSingleThreadExecutor(" not in ime or "preferencesWorker" in ime:
        errors.append("ImeDictionarySync 要在自己的工作线程上跑，不能排在 preferencesWorker 后面")
    service = strip_comments(read(SERVICE))
    for call in ("imeDictionarySync.start()", "imeDictionarySync.stop()", "imeDictionarySync.idle()", "imeDictionarySync.localChange()"):
        if call not in service:
            errors.append(f"MSIMEInputService 没有调用 {call}")
    if "NativeClient.dictionarySync(" in service:
        errors.append("MSIMEInputService 不能直接调用 NativeClient.dictionarySync，要经 ImeDictionarySync 的工作线程")
    page = strip_comments(read(PAGE))
    for trigger in ("TRIGGER_OPENED", "TRIGGER_MANUAL"):
        if trigger not in page:
            errors.append(f"DictionarySyncPage 没有用 {trigger}")

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(f"android dictionary sync contract: {len(rust)} error codes match, 2 MiB bound, shared directory, keyboard triggers")
    return 0


if __name__ == "__main__":
    sys.exit(main())
