# Agent Note: Linux 下载词库也带上内置辅助码

Status: implemented

## Problem

#8575 在两个 KDE/Fcitx5 环境均无辅助码。七套表的安装被 `MSIME_ENGINE_RESOURCES` 包住，而发布包不内置词库；首次配置只下载词库锁中的文件，辅助码永远缺失。

## Decision

码表无条件随包安装。首次配置与用户词库升级时，复制安装前缀的七个固定表到所选资源的 `helpcodes`，保留自定义文件；包内资源自身无需复制。目标不沿目录链接写入，文件以临时文件替换。升级在会话维护锁内补表。

## Alternatives considered

仅移动 CMake 安装块改动最小，但下载目录依旧读不到表。将表加入远程词库锁可复用下载，却增加联网与词库发布耦合，仍由引擎随包提供。

## Verification

合成前缀覆盖首次补表、重复执行、表更新、自定义文件保留及链接拒绝；`setup_resolution.py`、`setup_update.py` 与 `setup_registration.py` 通过。容器门禁在 full/wubi 无词库安装后逐字节核对七套表；本机 Docker 不可连接，未执行这项安装验收。

## Consequences

macOS 没有 KDE 运行环境，Docker 当前不可连接；系统输入法验收仍有缺口。

## 历史笔记检索

检索辅助码安装、`helpcodes`、`setup`。[五笔 86 笔记](../feature/2026-10-09-wubi86-helpcode.md)持有来源与分发清单，与本篇部分重叠并互链；自定义表扩展名和移动端导入不涉及 Linux 分发，无关本次决定。
