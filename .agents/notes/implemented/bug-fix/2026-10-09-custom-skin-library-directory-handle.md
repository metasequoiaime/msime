# Agent Note: 自定义皮肤库绑定目录句柄

Status: implemented

## Problem

自定义键盘皮肤库在状态目录锁内仍通过路径检查、读取和临时文件发布 `library.json`。目录替换竞态会让皮肤数据访问错误位置。

## Decision

读取复用共享私有文件入口；Unix 写入用已打开目录句柄和 `write_private_file_at` 原子发布；非 Unix 保留临时文件发布。加载路径也统一经过锁和同一读取逻辑。

## Alternatives considered

- 继续使用路径元数据加路径打开：检查与使用之间仍有竞态。
- 只保护 `library.json` 叶节点：不能阻止父目录被替换。
- 在皮肤库单独复制目录遍历：会让本地状态存储的安全策略分叉。

## Consequences

自定义皮肤的创建、更新、删除、加载都绑定到受信任目录；名称、容量和 JSON 校验行为不变。

## Verification

`cargo test -p msime-client-core --lib skin::custom_library --locked --quiet`：6 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
