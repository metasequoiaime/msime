# Security policy

请不要在公开 issue、pull request 或讨论中提交密码、访问令牌、真实输入、个人资料或其他敏感信息。

如果发现可能影响用户数据、凭据、输入内容或进程边界的安全问题，请通过 GitHub 的私下安全报告功能联系维护者：<https://github.com/metasequoiaime/msime/security/advisories/new>；不便使用 GitHub 时也可以发邮件到 admin@msime.app。报告中请提供受影响的提交或版本、复现步骤、影响范围和安全修复建议；请先避免公开披露细节。

仓库中的测试凭据和网络地址必须是合成值或本地 fixture。发布前请检查提交内容、构建产物和日志中没有真实秘密。

代码在什么条件下发出什么网络请求，逐项列在[网络请求与数据流向](../PRIVACY.md)；产品的隐私政策在 <https://msime.app/privacy/>。新增任何会把用户输入、音频或剪贴板内容送出设备的代码路径时，同时更新那份文档——一条没有记录的出网路径本身就是安全问题。
