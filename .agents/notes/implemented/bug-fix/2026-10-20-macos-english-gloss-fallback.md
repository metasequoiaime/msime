# Agent Note: macOS 独立输入法补充英文释义表回退

Status: implemented

## Problem

macOS 发布包把 `character-glosses` 和 `word-glosses` 放在外层 `MSIME.app` 的 `Contents/Resources` 中，但安装输入法时实际运行的是独立的 InputMethodKit bundle。输入法进程的 `EngineResources` 可能还来自旧配置目录，独立 bundle 又没有这两张表，因此 `酸奶` 这类不在小型 `msime-english.db` 里的中文候选没有英文释义。

## Decision

英文释义表同时复制到独立输入法 bundle 的 `Contents/Resources`。`InputController` 将该目录作为只读的 `supplementary_resources` 传给 Host API。Host API 查表时先使用 `EngineResources` 旁边的表；该表缺失时，再使用显式补充目录中的同名表。补充目录只接受绝对路径，并保持用户词典覆盖发布词典的现有优先级。

补充目录通过候选释义请求的 JSON 字段传递，不改变已有调用方的必填字段；字段缺失时行为保持不变。打包检查同时确认外层设置应用和嵌入的输入法 bundle 都带有对应的 NOTICE 文件。

## Alternatives considered

- **只打开或扩大默认英文释义开关**：改动小，但无法找回发布包中根本不存在的长尾词表，旧 `EngineResources` 仍然会得到空释义，因此不能解决问题。
- **只让 Host API 搜索外层设置应用的资源**：可以复用现有文件，但输入法进程不应依赖另一个应用 bundle 的安装路径；卸载、版本切换或独立输入法安装都可能使该路径失效。
- **只把表复制进输入法 bundle，不增加显式回退路径**：新安装可用，但旧配置仍优先使用旧 `EngineResources`，新表不会被读到；显式回退能兼容这类已有安装，同时不覆盖同目录的用户词典。

## Consequences

输入法 bundle 会多携带两张数据库及其 NOTICE，发布包体积略增，签名范围也包含新增资源。旧安装无需重新准备词库即可通过 bundle 资源获得 `酸奶 → yogurt` 等长尾释义；资源目录旁已有新表时仍优先使用它。Host API 的回退只作用于英文候选释义，逐词拆解等旧调用继续只读取其原有目录。

## Verification

- Host API 回归测试用合成的旧 `EngineResources` 和补充 `word-glosses` 表验证 `酸奶` 返回 `yogurt`。
- `package-release.sh` 的包检查在对应表存在时验证外层和嵌入输入法 bundle 的 NOTICE 文件。
- 运行 `cargo fmt --all`、定向 Host API 测试、`git diff --check` 和 `pnpm run verify-notes`。
