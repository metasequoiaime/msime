# Agent Note: iOS 快捷符号使用共享偏好并覆盖手机入口

Status: implemented

## Problem

#8558 的初稿把新设置规则放在 Swift/App Group，违背新偏好必须由共享层持有的边界；手机 26 键还强制显示逗号且关闭快捷菜单。长按与拖动各自复制光标逻辑，拼音长按错误地总按音节跳转。

## Decision

client-core 持有 `touch_quick_punctuation_symbols` 的默认值、字素校验和规范化，host-api 提供有界纯转换接口；Swift 只采集字符、调用共享 PreferencesStore 保存并镜像到 App Group。空配置恢复默认七个标点，最多十二个去重单字素，组合 emoji 保留。新设置当前只在 iOS 提供界面，其他宿主保留字段；共享 React 偏好对照门禁在 `RUST_ONLY` 中记录这项原生界面归属。macOS 偏好覆盖检查也在 `NOT_APPLICABLE` 中说明其屏幕键盘不读取该触屏列表，避免将有意的平台边界误报为宿主遗漏。

中文手机 26 键在选择了自定义列表时使用第一项键面和完整长按菜单，默认列表保留逗号长按句号；英文、日文和本地模式保持原集合。长按会话在 began 固定语音或光标路由，复用拖动的文档身份检查、步进及清理；长按慢速编辑按字母移动拼音，手写有墨迹时不吞空格。

## Alternatives considered

原先 App Group 方案不扩展共享文档且贴合现有手势开关，但复制了新设置的默认值与校验，违反架构约束，因此替代[初稿中的符号存储决定](../feature/2026-10-18-ios-space-cursor-and-quick-symbols.md)。只给九键接入列表改动小，但无法满足手机 26 键用户的自定义快捷符号需求。

## Verification

client-core 全套 400 项单元测试与 4 项集成测试、C ABI 回归及 client-core/host-api clippy 通过。模拟器 Rust 原生库与原生 MSIMEApp 编译通过，产品 App 安装启动成功；使用 ad-hoc 签名保留 App Group entitlement 后，QuickPunctuationSettingsTests 与 SpaceVoicePanelTests 共 9 项通过。macOS CI 的偏好覆盖用例暴露平台归属说明遗漏，补齐后使用同一 `preference_coverage.py` 检查确认。未做真机或第三方编辑器长按验收。

## Consequences

设置通过共享文件锁与修订号写入，App Group 只是缓存；增加一个共享偏好字段与纯 C ABI。当前没有给其他宿主增加设置界面。原生测试覆盖规范化、保存与手机键面菜单；模拟器测试宿主没有真实文本代理，不能据此声称第三方编辑器实测通过。

## 历史笔记检索

检索 iOS 空格、快捷符号、触屏偏好。初稿部分被替代并互链；设置界面归属笔记的共享层规则继续有效，不改变现有手势开关存储。
