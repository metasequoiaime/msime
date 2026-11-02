# Agent Note: PR 的截图改传自建图床 sugi.moe

Status: implemented

## Problem

2026-11-01 定下 PR 截图一律传 catbox。一天后就碰到了它的短板：catbox 对所有上传回「Uploads paused until I can resolve storage issues」，#10617 的截图传不上去，只能在正文里写本机路径。catbox 是一个人维护的免费站，没有 SLA；匿名上传连续 2 年无人访问就删；它自己的 FAQ 列了澳大利亚、伊朗、土耳其的屏蔽，以及 Comcast、Verizon 和 Quad9 过滤 DNS 的拦截。我们对它的可用性没有任何控制。

对图床的要求有四条：贡献者不用注册、不用密钥就能传；编码代理一条命令就能传；返回文件本身的直链，没有落地页和广告；图长期保留。逐个核对过 Imgur、ImgBB、Postimages、pixhost、0x0.st、pixeldrain、freeimage.host、gofile、litterbox、qu.ax，四条都满足的只有 catbox。

## Decision

自建一个满足同样四条的匿名图床 sugi.moe（Cloudflare Worker + R2，代码在 yldm-tech/sugi），PR 截图改传它。

- 上传：`curl -F file=@<图片> https://sugi.moe/upload`，返回 `https://sugi.moe/<内容哈希>.<扩展名>` 直链，以 Markdown 图片的形式嵌进正文；网页 https://sugi.moe 也能拖拽、粘贴上传。贴进正文之前把链接下载回来，确认是图片且与本地文件逐字节一致。
- 只收 PNG、JPEG、WebP、GIF，按文件内容判断类型，单张不超过 20 MB；同一内容总是同一个链接，已有文件不会被覆盖。
- 上传是公开、永久、匿名的，传错了贡献者自己删不掉：图里不能出现真实输入、账号、个人资料、本机路径或凭据。误传了发信到 abuse@sugi.moe 请维护者删。
- sugi.moe 暂时不可用时退回 files.catbox.moe 的直链；两者以外的匿名文件站（tmpfiles.org、file.io 一类）一律不用。
- 本机截图照常保留，上传的是另一份。

## Alternatives considered

- **继续只用 catbox。** 四条要求它都满足，但可用性完全在别人手里，这次暂停上传就是例子，旧 PR 里的图也可能随它一起失效。
- **专门建一个 GitHub 仓库存图，PR 引用 raw 链接。** 不经第三方、不会过期，但外部贡献者没有写权限，只有维护者能用，不满足「谁都能传」。
- **R2 存储桶加自定义域名，只给维护者上传权限。** 同样只有维护者能用。sugi.moe 用的也是 R2，区别在于前面有一个面向所有人的匿名上传入口。
- **GitHub 网页上的拖拽附件。** 谁都能用，但没有命令行接口，编码代理做不到。

## Consequences

- 收益：图床在自己的域名和账号下，不受第三方停服、清理和屏蔽影响；上传方式和 catbox 一样一条 curl。
- 代价：要维护一个对公众开放的匿名上传服务，处理滥用举报是我们自己的事。为此 sugi.moe 用了与 msime.app 无关的独立域名，只收图片并以禁止脚本的响应头返回，按 IP 限速（Worker 绑定加 zone WAF 规则），开了 Cloudflare CSAM 扫描，记录上传来源的 HMAC 以便按来源清除，举报地址 abuse@sugi.moe 转发给维护者。
- 没有脚本检查 PR 正文是否附图，靠 `AGENTS.md` 和 PR 模板提示执行。

[历史快照：PR 的截图上传到 catbox 并嵌进正文](../../archived/process/2026-11-01-pr-images-on-catbox.md)
