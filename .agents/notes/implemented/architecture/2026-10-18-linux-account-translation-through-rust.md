# Agent Note: Linux provider 的「水杉账号」释义改调 msime_client_account 的 translate

Status: implemented

## Problem

`msime-linux-online-provider` 的账号释义原来分四步拼在 Python 里：先 `ensure_anonymous`（每次释义都调，注册不成就整页没有释义），再经 `msime_client_account_access_token` 取令牌，用 `msime_client_account_gloss` 的 `request` 拿请求体，自己的 HTTP worker 发到写死的 `https://api.msime.app/v1/translate`，最后把正文交回 `parse`。结果是：

- 匿名账号注册失败时，已经登录的用户也拿不到释义，因为注册排在取令牌前面。
- 服务端拒绝令牌时 Python 不带 `rejected_token` 重取，也不重试；匿名会话被吊销以后，`ensure_anonymous` 看见文件还在就不联网，provider 一直拿那枚被拒的令牌去问，直到访问令牌自己过期（最长 15 分钟）、刷新被拒、Rust 清掉会话以后，下一次注册才补上。
- 状态码只当 200 处理（worker 对非 2xx 直接失败），限流、5xx 和 401 分不开。
- 服务端地址、bearer 和超时在 Python 里又有一份。

client-services 早有整条做完的入口：`msime_client_account` 的 `translate`（登录的账号优先，没有登录时用本机匿名账号，401 时在同一个账号里刷新重试一次，逐项规范化与 `msime_client_account_gloss` 相同）。Windows 因为要在 Server 退出时中止请求，留在 sans-IO 那一半（见 [Windows 账号释义改用 msime_client_account_gloss](2026-10-17-windows-account-gloss-sans-io.md)）；Linux provider 是独立进程，没有这个约束。

## Decision

- `RustAccountApi.translate(texts, source, target, deadline)` 调 `msime_client_account`：`{directory, client_agent?, action:{operation:"translate", texts, source_lang, target_lang, timeout_ms}}`。`timeout_ms` 取这一次 `translations` 剩下的期限，夹在 `ACCOUNT_TRANSLATE_TIMEOUT_MS`（1000–2500）之间：上限是原来 Python 发请求时的 2.5 秒，下限是 Rust 接受的最小值；期限已过时不调。`client_agent` 与注册时同一个（头文件要求同一个宿主的各次调用都带同一个）。
- 回 `account_unauthorized`（两种会话都不可用：安装时的注册没成功、文件被删、匿名会话被吊销后已被 Rust 清掉）时补注册匿名账号（`msime_client_ensure_anonymous_account_with_agent`），成功后再问一次。`ANONYMOUS_REGISTRATION_RETRY`（600 秒）之内一个进程只补一次，按尝试计、不按成功计，与 Windows 的 `account_registration_retry` 相同。其他错误不补注册，这一组词记作没有释义，照旧进 480 秒的否定缓存。
- 回复的 `translations` 与 `texts` 等长才用，每项不是非空字符串就是没有释义；长度不对整组作废。
- 删掉 `access_token`、`prepare_translation`、`parse_translation`、`ANONYMOUS_ACCOUNT_ORIGIN` 和到 `/v1/translate` 的 `fetch`；`anonymous_translation` 改名 `account_translation`。必需符号收成 `HOST_LIBRARY_SYMBOLS`：`msime_client_ensure_anonymous_account_with_agent`、`msime_client_client_agent`、`msime_client_account`，少一个就当作没有可用的库。
- `--ensure-anonymous-account`（deb 的 postinst 和 `msime-linux-setup` 调）不变，不受 600 秒限制。

## Alternatives considered

- **保留 sans-IO（`msime_client_account_gloss`），只补上 401 重取和状态码** — 与 Windows 一致，HTTP 仍走 provider 那个带硬期限、父进程能杀掉的 worker 子进程；但要在 Python 里再写一遍「被拒的令牌带回去强制刷新、只在同一个账号里重试」，而这正是 `with_shared_access_token` 已经有、Windows 因为 `stopping()` 不得不在 C++ 里重写的那一段。Linux provider 退出时只是丢掉在途的请求，不需要中止它。
- **每次释义之前照旧先 `ensure_anonymous`** — 第一次释义前就有匿名账号，不用等一次 `account_unauthorized`；但已有会话时它也要读一次文件，注册不成时会挡住已经登录的用户，且对被吊销的会话无效（文件在就不联网）。

## Consequences

- **收益**：登录的用户不再受匿名注册失败影响；服务端吊销令牌后，同一次请求里 Rust 先刷新，刷新也被拒时清掉会话，provider 补注册后再问，下一页（或同一页）就恢复；限流、5xx、断网和 401 由 Rust 分开，provider 只认 `account_unauthorized` 去补注册。服务端地址、bearer 和超时只剩 Rust 一份。
- **代价与已知上限**：`timeout_ms` 不约束刷新令牌和补注册那两步，一次释义在刷新或注册很慢时可能超过 provider 的 6 秒期限，结果被引擎那边 8 秒的 socket 期限丢掉，下一页再问。补注册失败以后 10 分钟内这个进程不再试，没有会话的用户这段时间没有账号释义。缓存键仍按账号目录分，不按 user.id，换号后已经缓存的释义照用（释义与账号无关，与原来相同）。

## Verification

- `platforms/linux/tests/dictionary/translation_provider_selection.py` 的 `AccountTranslation`：请求形状、`timeout_ms` 随期限夹在 1000–2500、有会话时不注册、`account_unauthorized` 时补注册一次再问、600 秒内不重复补、其他错误不补、长度不对整组作废；`TranslationProviderSelection` 确认选了「水杉账号」时 provider 自己的 `fetch` 一次也不调。
- `platforms/linux/tests/provider/host_library_contract.py`（列在 `python-contracts.list`，legacy 包在 Python 3.7 上也跑）：`HOST_LIBRARY_SYMBOLS` 每个都在 `msime_client.h` 里是 `char *f(const uint8_t *, size_t)`，provider 源码里不再有 `msime_client_account_access_token`、`msime_client_account_gloss` 和服务端地址，`msime_voice_paths.py` 的三项由 CMake 写出并装进 `bin`，注册与释义带同一个 `client_agent`。设了 `MSIME_HOST_LIBRARY` 时再用真的库核对 Rust 收下 provider 拼的 `translate` 请求（空目录里答 `account_unauthorized`，请求不合法时是 `account_invalid`）和 `client_agent` 请求。
- `scripts/test-cloud-request-budget.py` 盯住 provider 经 `msime_client_account` 释义、不再取令牌。
- 相关笔记：[Linux provider 按安装路径加载 host 库，匿名注册自报 User-Agent](../bug-fix/2026-10-17-linux-provider-host-library-and-agent.md)。
