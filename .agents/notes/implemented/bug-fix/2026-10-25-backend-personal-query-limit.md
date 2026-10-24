# Agent Note: 个人查询先验证候选上限再扩展调序窗口

Status: implemented

## Problem

后端协议的 `Request::new` 统一要求 `limit` 在 `1..=200`。个人查询为了在固定位置调序前取得足够候选，把嵌套查询的 `limit` 改成 200 后才构造 `Request`；原值只检查整数类型，0、负数与大于 200 的整数绕过公共范围校验。真实协议红例中，`personal_query` 的 `limit=0` 返回正常空候选，而不是 `invalid_request`。

动手前检索活跃笔记中的 `personal_query`、`MAXIMUM_REQUEST_BYTES` 与后端协议，没有既有归属；客户端拆层笔记只解释后端进程的依赖图，与本次请求边界无关。

## Decision

先用现有 `Request::new` 验证未经修改的嵌套查询，从 `limit()` 取得已验证的上限。随后仍以 200 个候选执行个人调序，最后按调用方的有效上限截断。三个个人请求入口共用这一边界，不另写一套数值范围或默认值。

## Alternatives considered

- **单独检查原始整数是否在范围内。** 改动局部且容易看懂；不采用，因为会重复 `Request::new` 的协议规则，默认值或上限以后可能分叉。
- **直接用调用方上限查询。** 可少读取候选；不采用，因为固定位置调序可能需要原排名在调用方窗口之外的词，不能为参数校验改变现有排序语义。

## Testing

原有合成 SQLite 资源和真实协议入口复现旧实现的 `limit=0` 红例。修复后的全部后端协议测试 32 项通过、0 忽略：越界矩阵覆盖三个个人请求入口的 0、负数、201 与整数极值；有效值覆盖 1、200 和缺省上限，并验证原先排在返回窗口外的词仍先调到固定位置，再按请求上限截断。

复核命令为 `cargo test --locked -p msime-engine backend::tests::`、`cargo clippy --locked -p msime-engine --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 和 `bash scripts/verify-local.sh --quick`。

## Consequences

以前容忍的越界整数改为明确拒绝；有效范围、响应格式和调序算法沿用原契约。嵌套请求增加一次复用公共解析器的检查，内部调序窗口仍为 200。不增加依赖，不修改文件读写实现。
