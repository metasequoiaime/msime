# Agent Note: macOS 独立输入法补充英文释义和音标表回退

Status: implemented

## Problem

macOS 发布包把 `character-glosses` 和 `word-glosses` 放在外层 `MSIME.app` 的 `Contents/Resources` 中，但安装输入法时实际运行的是独立的 InputMethodKit bundle。输入法进程的 `EngineResources` 可能还来自旧配置目录，独立 bundle 又没有这两张表，因此 `酸奶` 这类不在小型 `msime-english.db` 里的中文候选没有英文释义。音标表原先只在外层设置应用，独立输入法的打包循环漏了 `pronunciations`；旧配置仍指向用户目录时，读音请求也没有补充资源目录，开启 `candidate_pronunciation` 仍查不到音标（#9407）。

## Decision

两张英文释义表与 `pronunciations` 音标表同时复制到独立输入法 bundle 的 `Contents/Resources`；最终包检查确认内嵌音标数据库和 NOTICE 都存在。`InputController` 将该目录作为只读的 `supplementary_resources` 传给 Host API。Host API 查表时先使用 `EngineResources` 旁边的表；该表缺失时，再使用显式补充目录中的同名表。补充目录只接受绝对路径，并保持用户词典覆盖发布词典的现有优先级。

补充目录通过候选释义和读音请求的 JSON 字段传递，不改变已有调用方的必填字段；字段缺失时行为保持不变。读音请求优先使用活动资源旁的 `pronunciations/en-phonetic.db`，只在文件缺失时回退到 `<supplementary_resources>/pronunciations/en-phonetic.db`；显式路径最多 4096 字节，空路径、相对路径和过长路径拒绝。两处都缺表时仍返回空读音，不改写配置或复制数据库。读取继续经过原有 SQLite 文件校验。打包检查同时确认外层设置应用和嵌入的输入法 bundle 都带有对应的 NOTICE 文件。

## Alternatives considered

- **只打开或扩大默认英文释义开关**：改动小，但无法找回发布包中根本不存在的长尾词表，旧 `EngineResources` 仍然会得到空释义，因此不能解决问题。
- **只让 Host API 搜索外层设置应用的资源**：可以复用现有文件，但输入法进程不应依赖另一个应用 bundle 的安装路径；卸载、版本切换或独立输入法安装都可能使该路径失效。
- **只把表复制进输入法 bundle，不增加显式回退路径**：新安装可用，但旧配置仍优先使用旧 `EngineResources`，新表不会被读到；显式回退能兼容这类已有安装，同时不覆盖同目录的用户词典。

## Consequences

输入法 bundle 会多携带两张释义数据库、一张音标数据库及其 NOTICE，发布包体积略增，签名范围也包含新增资源。旧安装无需重新准备词库即可通过 bundle 资源获得 `酸奶 → yogurt` 等长尾释义；资源目录旁已有新表时仍优先使用它。Host API 的回退作用于英文候选释义和英文音标，逐词拆解等旧调用继续只读取其原有目录。

## Verification

- Host API 回归测试用合成的旧 `EngineResources` 和补充 `word-glosses` 表验证 `酸奶` 返回 `yogurt`。
- `package-release.sh` 的包检查在对应表存在时验证外层和嵌入输入法 bundle 的 NOTICE 文件。
- 运行 `cargo fmt --all`、定向 Host API 测试、`git diff --check` 和 `pnpm run verify-notes`。
- #9407 的 FFI 回归先以 `invalid pronunciation request` 实际失败，确认旧 API 拒绝补充目录；修复后两项读音 FFI 测试通过。合成 `sky` 表覆盖缺省空结果、随包表回退、活动表优先与无效路径；Clippy 通过。
- #9407 分支在 API 28 专用模拟器运行 `build-core-test.sh`，四组共享库单测共 2035 项通过、0 失败；按脚本跳过 Android 不支持的硬链接前置条件与已有忽略项。macOS 原生 `candidate-pronunciation`、`candidate-translation-display`、`candidate-item-layout`、`candidate-row-layout` 共 4 项通过。
- 完整 quick 的冷编译运行在 `voice language catalog` 的 120 秒 Cargo 超时处失败；其后 Rust、Android、wasm、Linux 原生 111 项及 macOS 编译均完成。暖缓存整轮重跑仍失败：`linux-ibus-launcher-supervisor` 的第三次间隔为 9.821 秒，超过原测试 9.5 秒上限。没有放宽时限；隔离临时目录并降低契约检查并行度后的整轮 quick 通过，Linux 原生 111 项全部通过（`issue-9407-quick-isolated.log`）。
- 带固定词汇书的本分支 APK 在 API 28 安装成功，前三项设备冒烟通过，组字重启用例失败；单独重跑越过该断言，随后在放大拼写的长输入夹具处失败。两轮均不能记为完整冒烟通过，也尚未确定是基线问题还是回归。实际 macOS 输入法候选行尚未装机验收。

- Android 完整冒烟尚未通过：修复版首次停在组字重启，单独重跑通过原失败点后停在长拼写夹具。随后从本分支原始基线 `70debe5b77da269d5c00b740328ae63bb9c66762` 独立构建 APK，完整冒烟通过组字重启及长拼写等前 18 项，停在剪贴板导出选择器的合成控件检查（`issue-9407-api28-exact-baseline-smoke.log`）。失败位置不同，不能据此将修复版失败全部归为基线；已通过的 2035 项设备核心测试和 4 项 macOS 原生测试也不代表完整宿主验收。

- #9407 打包回归提取并执行真实的补充表复制循环，以合成数据库和 NOTICE 验证独立输入法包；未补 `pronunciations` 时明确失败，修复后通过。此检查不替代签名完整发布包或已安装输入法验收。
