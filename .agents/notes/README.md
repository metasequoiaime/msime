# 决策笔记

本目录按 write-notes-like-deepseek 维护跨会话决策笔记。方法、格式与「什么改动必须写」的判定标准见 [../skills/write-notes-like-deepseek/SKILL.md](../skills/write-notes-like-deepseek/SKILL.md)。

- 路径即状态：`{proposed|implemented|rejected|archived}/{feature|bug-fix|simplification|architecture|process|testing}/yyyy-mm-dd-主题.md`
- 不设 INDEX.md：目录位置本身就是状态，检索用 ripgrep 直接搜本目录。
- 校验：仓库根执行 `pnpm run verify-notes`；旧决定被取代时用 `pnpm run archive-agent-note` 归档封印。
