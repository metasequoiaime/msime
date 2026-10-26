# Agent Note: Android 我的皮肤显式导入导出操作

Status: implemented

## Problem

#9752：我的皮肤只能长按删除，没有单个导出或显式删除按钮，也没有从文件导入皮肤的入口，操作发现性差。

## Decision

复用 `CustomSkinLibrary` 已有的 JSON 导入导出格式和边界。在“我的”页面增加导入按钮；每张自定义皮肤卡片增加导出、删除两个显式按钮。导出通过 SAF `CreateDocument` 写单项 JSON，导入通过 `OpenDocument` 读取有界 JSON 并合并入库。保留点击卡片切换皮肤，移除长按删除路径。

## Alternatives considered

- **继续使用长按删除并只增加导入**：不能满足用户要求的显式删除和单项导出。
- **为皮肤另建文件格式**：会与本地备份、云同步和已有库格式分叉，增加迁移与校验成本。

## Consequences

按钮占用卡片底部空间，但删除和导出可发现性更好；导出只包含设计参数，不包含照片，和现有库格式一致。导入沿用库的容量、名称、JSON 和路径安全校验。尚需在 Android 模拟器上触摸验收 SAF 选择器、单项导出和导入结果。

## Verification

`bash platforms/android/check-host.sh` 和 `git diff --check` 已通过；待运行仓库 quick 门禁、笔记校验和 Android 设备验收。
