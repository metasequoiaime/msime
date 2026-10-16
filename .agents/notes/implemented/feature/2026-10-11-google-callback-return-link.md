# Agent Note: Google 登录回跳页在手机上加「回到水杉输入法」按钮

Status: implemented

## Problem

鸿蒙没有 Google 服务，Google 登录在系统浏览器里完成，跳回 App 在 127.0.0.1 上的监听，浏览器停在 `client-core` 写回的回跳页上：「已收到 Google 授权，请回到水杉输入法，登录会在那里完成。可以关闭此页面了」。手机上浏览器占满屏幕，页面上没有任何能回去的东西，用户得自己去多任务里找回应用。仓库负责人在模拟器上走完登录后要求加一个回到 App 的按钮。

## Decision

- `google_loopback_reply` 多一个可选参数 `return_link`：宿主注册的、能把应用拉回前台的链接。给了就在消息下面画一个「回到水杉输入法」按钮（`<a class="return">`，强调色底，文字取卡片色，深浅色都可读）。成功、取消、失败三种页面都有；不是回跳的 404 页没有。
- 链接只收 `scheme://路径` 的形状（`valid_return_link`）：scheme 小写字母开头、只含 `[a-z0-9+.-]`，不是 http、https、javascript、data、file 这些网页用的协议；路径只有字母、数字和 `/-_.`；总长不超过 128。所以它原样写进 `href` 不需要转义，也不会把浏览器引去网页。不合格时不画按钮，页面照常写回，不能因为这个顺带的按钮丢掉登录。
- 桌面的监听传 `None`：窗口就在旁边，页面和原来一样。FFI 的 `reply` 请求多一个可省略的 `return_link` 字段，头文件同步说明。
- 鸿蒙传 `msime://auth/google`，和 iOS 正式版的 scheme 一致。`module.json5` 给 EntryAbility 加一个 `entity.system.browsable` + `ohos.want.action.viewData` 的技能，接 `msime://auth`。拉起的 want 不带页面参数，`onNewWant` 什么也不做，应用只是回到前台，登录在应用里接着完成。
- 页面的 CSP 不变：点链接是用户发起的导航，不受 `default-src 'none'` 管。

## Alternatives considered

- **页面自动跳回应用**（`<meta http-equiv="refresh">` 或脚本）。页面的 CSP 不允许脚本；自动跳转在部分浏览器上会弹「是否打开应用」，用户没点就被打断，而且登录结果要等应用完成后才知道，自动跳走也看不到页面上的状态。
- **用 HTTPS 的 App Linking 代替自定义 scheme。** 要在域名上放关联文件、在 AppGallery 配置，改动大；只是把用户带回前台，自定义 scheme 足够。

## Consequences

- 鸿蒙的 ArkTS 和原生库必须同一次构建：FFI 的请求是 `deny_unknown_fields`，新的 ArkTS 带上 `return_link` 时，旧的 `libmsime_host_api.so` 会拒绝整个请求，Google 登录失败。正式包两者来自同一个提交，没有这个问题；本地拿旧的库打新的包时要先重编原生库。
- 别的应用或网页也能用 `msime://auth/...` 拉起本应用，但它只会把应用带回前台，不带任何参数，也不触发任何操作。

## Verification

- `crates/client-services/src/account/tests.rs`：有链接时三种回跳页带按钮、Content-Length 正确，404 页不带；网页协议、带引号或空格、大写 scheme、没有 `://`、超长的链接都不画按钮，回跳照常是 `Code`。
- `crates/host-api/src/ffi/google.rs`：`reply` 带 `return_link` 时回复里有按钮，不带时没有。
- 模拟器上的验证见 PR。
