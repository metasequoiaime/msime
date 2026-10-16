## 摘要

<!-- 说明用户可见的问题和改动后的行为。保持范围聚焦。 -->

## 影响范围

- [ ] 共享 Rust / client-base、client-core、client-services、client-providers
- [ ] React / Tauri 界面
- [ ] Android
- [ ] iOS
- [ ] macOS
- [ ] Linux
- [ ] Windows TSF / Server
- [ ] HarmonyOS
- [ ] 文档或构建工具

## 验证

<!-- 列出实际执行的命令和平台检查，写清楚跑在哪个平台、哪个环境上。各平台的验证入口见 docs/ARCHITECTURE.md。 -->

- [ ] `bash scripts/verify-local.sh --quick`
- [ ] `git diff --check`
- [ ] 下面列出相关的测试、类型检查、fmt、clippy 和原生检查

## 安全与发布

- [ ] 不包含真实输入、凭据、个人资料、私人路径、生成产物或签名材料。
- [ ] 新引入的上游代码、资源、模型和依赖都附有来源与许可证/通知信息。

<!-- 安全漏洞请走私下的 security advisory 流程，不要开公开 PR。 -->
