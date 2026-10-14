# Agent Note: Android 候选释义按来源过滤

Status: implemented

## Problem

共享层 `source_gets_no_gloss` 排除来源 6（表情）、7（颜文字）、13（日文混输）和 14（插件）。Android 的用户翻译路径已经跳过这些来源，但账号翻译路径仍把整页候选送出；合并译文时也按文字而不是来源应用。候选条、展开面板和长按菜单直接读取 `translation`，所以特殊来源可能显示或上屏账号译文。

## Decision

Android 统一使用候选来源门控：账号和用户服务请求只带允许来源，在线与离线译文合并时不把禁止来源加入请求文字集合，候选条、读屏、展开面板和长按菜单消费译文前再次检查来源。这样同一文字同时出现在普通候选和特殊来源候选时，普通行仍可显示，特殊行不会复用译文。

## Alternatives considered

- **只在账号请求前过滤**：可以减少网络请求，但已经写入共享视图的译文仍会被候选条和长按菜单消费，且同词候选会跨来源复用。
- **把来源加入 Android 缓存键**：会让同一普通文字重复请求，离线和共享视图仍需要单独过滤；本次在请求与每个消费入口都执行来源门控。

## Consequences

禁止来源不再消耗账号或用户翻译服务额度，也不再显示或插入候选释义。共享运行时仍按文字保存译文，Android 的每个显示与输入入口都按候选来源过滤，因此同词候选不会越过来源边界。

## Verification

Android host smoke 将覆盖来源策略；同时运行相关 Java smoke、`bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check`。
