# Agent Note: Android 设备 workflow 的 artifact action SHA 打错了一位

Status: implemented

## Problem

#11268 的 nightly 运行中，Android API 28 和 API 35 设备 job 在任何设备步骤开始前就失败：`Unable to resolve action actions/upload-artifact@cf430a030ddbb5b0abf93d22962f4752f3646cd9`。Android 0.7.4 的发布运行（43054260089）也因为同样的原因失败，两个设备 job 没起来，发布、蒲公英和 Play 三步全部跳过。

原因是 `android-device.yml` 第 185、198 行把 v7.0.2 的提交写错了一位：`cf430a0…`，而 `actions/upload-artifact` 的 `v7.0.2` tag 指向 `cf430e030ddbb5b0abf93d22962f4752f3646cd9`。GitHub 对 `cf430a0…` 返回 `No commit found`。仓库其余 66 处引用都是正确的 `cf430e0…`，同一文件的另外四处也是，所以只有这两个步骤所在的 job 解析失败。

## Decision

只把这两处改成正确的 `cf430e030ddbb5b0abf93d22962f4752f3646cd9`（`v7.0.2`），与仓库其余引用一致。用 `gh api repos/actions/upload-artifact/git/ref/tags/v7.0.2` 核对了 tag 指向的提交，用 `gh api repos/actions/upload-artifact/commits/<sha>` 确认错写的 SHA 不存在。

## Alternatives considered

- **把这个文件的六处都降到 `v7.0.1`（`043fb46…`）**：这个 PR 最初的做法。能修好，但前提记错了（以为 `v7.0.2` 本身无法解析），而且让这个文件和仓库其余 66 处版本不一致，以后升级 action 时容易漏，因此改成只纠正错字。
- **重跑 nightly 或发布**：错写的 SHA 永远解析不了，重跑没有用。

## Consequences

Android 设备 job 恢复，发布工作流的设备冒烟测试能跑起来。以后手写 action SHA 时要从 tag 查出来再复制，不要手敲；同一个 action 在仓库里出现多个 SHA 时先查是不是打错了。
