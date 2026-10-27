# Agent Note: AI 候选和候选翻译的 curl 头部链表被提前释放

Status: implemented

## Problem

Windows 0.3.0 的 Server 在 libcurl 里报 `EXCEPTION_ACCESS_VIOLATION (0xc0000005)`，崩溃签名 `6439c6634e108669`，首次出现在 2026-10-21（0.3.0-beta），近 7 天 317 次、45 台设备。用 `windows-symbols` 上 full 0.3.0 的 `MetasequoiaImeServer.pdb` 还原，栈底是 `AiCandidateWorker::run` → `AiCandidateWorker::fetch` → 匿名命名空间的 `https_post` → `curl_easy_perform`。libcurl 一侧没有 PDB，用 beta 构建产物里同一份 vcpkg curl 8.14.1 的 `libcurl.dll` 反汇编，按各函数引用的字符串认出是 `multi_runsingle` → `Curl_http`（拼请求）→ `Curl_add_custom_headers`（遍历 `CURLOPT_HTTPHEADER`，引用 `Host`、`Content-Type`、`Authorization`、`Cookie` 等字符串的那个函数），崩在读头部的小函数里。

`https_post` 构造头部链表时每追加一项就 `request_headers.reset(raw_headers)`。`curl_slist_append` 返回的是表头，从第二项起它就是 `unique_ptr` 已经持有的指针，而 `unique_ptr::reset` 收到同一个指针也会先删除旧值，于是 `curl_slist_free_all` 把整条表释放了，后面的追加和 `curl_easy_perform` 用的都是悬空链表。AI 请求至少带 `Authorization` 和 `Content-Type` 两个头，每次请求都踩到；崩溃地址不固定（`libcurl.dll+0x531db`、`+0x705e4`），因为释放后的内存何时被谁重用并不确定。`TranslationWorker.cpp` 的 `https_post` 是同一段代码，候选翻译走同样的路径。`CloudCandidateWorker.cpp`、`shared/voice/VoiceProviders.cpp` 和 `compat/Windows7WebSocket.h` 只在追加成功后接管表头或从不 reset，不受影响。

## Decision

两处共用的头部校验与链表构造提到 `platforms/windows/src/candidate/CandidateCurlHeaders.h` 的 `candidate_curl_headers`：逐项校验名字与值（规则不变，仍用 `CandidateHttpPolicy.h`），追加失败返回 `nullopt`，只在链表还空时让 `unique_ptr` 接管表头。`AiCandidateWorker.cpp` 和 `TranslationWorker.cpp` 改为调用它。`tests/candidate/ai_candidate_worker.cpp` 加一段检查：三个头部全部按顺序留在链表里，空对象得到空链表，非法名字、带换行的值和非字符串值被拒绝。

## Alternatives considered

- **两处就地改成 Cloud 那样的写法，不加共用函数**：改动最少，但同一段 15 行的代码留两份，正是这次一处写错、另一处照抄的原因，而且匿名命名空间里的代码测不到。
- **把头部构造放进 `CandidateHttpPolicy.h`**：那个头文件现在只依赖标准库，设置窗口等不链 curl 的目标也引入它，放进去会让它们多依赖 curl 与 nlohmann。

## Consequences

AI 候选和候选翻译的请求头部链表在整个请求期间有效，Server 不再因此崩溃。云候选仍用自己那份写法，没有并到共用函数里。

## Verification

在 macOS 上用 clang++ `-fsanitize=address` 把 `CandidateCurlHeaders.h` 和系统 libcurl 编成一个小程序：新函数构造的三个头部完整、按顺序，退出码 0；同一程序里原样照搬的 0.3.0 写法在第二次追加后读到已释放的链表，ASan 报 SEGV。这只验证了链表本身，没有在 Windows 上复现原崩溃；`windows-ai-candidate-worker` 测试的新段落与两个 Worker 的 MSVC 编译交给 PR 的 Windows CI。发版后需要看签名 `6439c6634e108669` 在新版本上是否归零。
