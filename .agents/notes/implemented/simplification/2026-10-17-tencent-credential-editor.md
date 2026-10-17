# Agent Note: 腾讯云凭据编辑复用公共纯模型

Status: implemented

## Problem

`LinuxTencentCredentialsSection` 与 `useProviderCredentials` 分别声明同一临时输入结构，表单还内嵌地域回退、凭据状态说明、按钮禁用和保存组装。独立调用方必须依赖展示组件的类型，或重新维护这些规则。

## Decision

在无 React 依赖的 `tencent-credential-model.ts` 统一 `TencentCredentialInput`、宿主状态投影和保存参数，公开 `tencentCredentialEditorModel` 与 `tencentCredentialSavePayload`。表单只绑定模型和字段回调，凭据状态 hook 直接引用公共输入契约。旧 hook 与 Linux 组件路径继续转导出，翻译设置组合层直接引用模型类型。

地域沿用 `input.region ?? status.tencent.region ?? "ap-guangzhou"`：显式空串覆盖已存地域，由宿主维持原有默认处理。空白 SecretId、SecretKey 省略提交，非空值保留原文；已有凭据允许只更新部分字段，新凭据仍要求两个字段。地域格式只决定更多选项默认展开，不新增保存阻断。

## Existing notes audit

- [翻译凭据偏好公共模型](2026-10-16-translation-credential-preferences.md) 归属共享偏好字段与服务测试投影；本篇归属 Linux provider 凭据文件的编辑输入和保存组装，两者存储入口不同。
- [表达页精简](2026-10-16-expression-page-prune.md) 继续拥有 Linux 地域折叠与服务选择的理由，本片直接复用既有地域校验，并互链编辑模型归属。
- [语音凭据编辑模型](2026-10-17-voice-credential-editor.md) 提供纯模型与原文保存的既有模式；腾讯云表单没有本地状态，使用纯函数承接规则。

## Alternatives considered

- **只合并输入类型**：变动最小，但地域回退、状态说明和保存组装仍留在组件，公共调用方继续复制规则。
- **增加公共 React hook**：能提供统一字段动作，但腾讯云编辑只转发受控输入，没有本地生命周期或展开状态，纯函数已足够表达现有规则。

## Verification

公共模型测试先在缺少入口的旧实现出现 5 项断言失败；实现后 6 个相关测试文件共 33 项通过，覆盖地域回退、显式空串、状态说明、保存条件、原文与空白字段省略、忙碌状态、能力不可用以及凭据草稿和翻译绑定。类型兼容测试覆盖旧深路径、包入口和宿主保存参数。独立只读审查通过，审查方另跑 2 个核心文件共 11 项测试通过。desktop 类型检查与构建、HarmonyOS 类型检查、格式、lint、笔记检查和本地 quick 门禁均通过；lint 保留未涉及文件的已有警告。

## Consequences

- 表单和凭据状态 hook 共用输入类型，公共调用方可直接使用地域回退、展示模型和保存组装；旧入口仍可使用。
- 原有地域折叠、密钥省略与原文保存行为保持一致，模型只接收已存地域和损坏标记，不持有已存秘密。
- 公共函数增加 API 维护面；调用方仍负责受控输入和异步保存，原生持久化校验由共享 Rust 和宿主负责。
- 本片验证范围是共享 React 编辑层，不代表原生宿主装机或真实翻译服务验收。
