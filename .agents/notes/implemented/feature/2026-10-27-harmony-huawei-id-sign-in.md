# Agent Note: 鸿蒙用华为账号登录替换 Google 登录

Status: implemented

## Problem

华为应用市场（中国大陆）审核鸿蒙版时，审核员点「使用 Google 登录」没有反应：中国大陆连不上 Google，系统浏览器打不开授权页。审核意见建议提供「华为账号登录」。鸿蒙版的第三方登录只有 Google 一种（[鸿蒙通过系统浏览器加本机回环提供 Google 登录](2026-10-09-harmony-google-sign-in.md)），那篇笔记当时就记下这个按钮对中国大陆用户没有用、按地区隐藏没有决定。维护者决定：鸿蒙上不再给出 Google 登录，改为华为账号登录，其他平台不变。

## Decision

客户端经 HarmonyOS Account Kit 拿华为账号的 ID Token，服务端（msime-cloud）像 Apple、Google 一样只验 ID Token；身份按 ID Token 的 `sub`（即 UnionID）记。

- **服务端**（msime-cloud `internal/account/providers.go`）：通用 OIDC 校验表加 `huawei` 一项，发行方 `https://accounts.huawei.com`，公钥 `https://oauth-login.cloud.huawei.com/oauth2/v3/certs`，受众是配置 `auth.huawei.client_ids` 里的应用 Client ID，签名算法接受 PS256 和 RS256（Account Kit 缺省 PS256，JWKS 里的 RSA 公钥两种都用）。nonce 照旧必须等于 challenge 的 nonce。`/v1/auth/providers` 多一个 `huawei` 开关。不换授权码，不需要 Client Secret。
- **共享 Rust**：`client-services` 的 `huawei_challenge`（不带目标、必须拿到 nonce）和 `sign_in_huawei_id_token`（只放行三段 base64url 的 JWT），`/v1/auth/providers` 的 `huawei` 进 `ProvidersResponse`，资料页的登录方式名称表加「华为账号」。`host-api` 的 `msime_client_account` 加 `huawei_challenge`、`huawei_login` 两个宿主操作，与 Apple 原生登录同形，没有 `purpose`（不支持绑定）。
- **鸿蒙宿主**：`account/HarmonyHuaweiSignIn.ets` 经 Rust 申请 challenge，用 `AuthorizationWithHuaweiIDRequest`（`scopes: ["openid"]`、`permissions: ["idtoken"]`、`forceAuthorization: true`、nonce 原样传入、`idTokenSignAlgorithm: PS256`）拉起系统授权页，核对 state 后把 ID Token 经 `huawei_login` 交回 Rust。可测的判定在 `HuaweiIdSignInPolicy.ts`。页面只能发 `huawei_sign_in`，它和 `google_sign_in` 一样算换会话的操作，前后取消旧账号排队中的词库快照；`huawei_challenge`、`huawei_login` 不在页面的操作名单里。
- **共享设置页**：`AccountProviders.huawei` 加 `AccountClient.huaweiLogin`，「华为账号登录」只在服务端开着且宿主提供 `huaweiLogin` 时出现，手机的登录面板和 2in1 的登录卡片都有，点击先过同一个同意勾选（`useLoginConsent`，#10009）。按钮按华为账号登录按钮规范写「华为账号登录」，文字左边是华为标志。ArkWeb 里用不了 SDK 的 `LoginWithHuaweiIDButton` 组件，按钮由页面画；标志起初没有放，2026-10-30 补上，用的是 SDK 里那个组件自己画的两张 HMS 资源（`packages/ui/src/assets/huawei-id/`，来源见 `docs/third-party.md`），浅色主题红色版、深色主题白色版。
- **隐藏 Google**：`apps/harmony/src/main.tsx` 用 `HARMONY_GOOGLE_SIGN_IN = false` 不给页面 `googleLogin` / `googleCancel`，共享页面已有的门控（服务端开着且宿主提供）于是不画 Google 按钮。`HarmonyGoogleSignIn.ets`、`google_sign_in` 桥接和 Rust 的回环判定原样保留，改回 true 就恢复；其他平台的宿主不经过这个开关。

### 为什么按 UnionID 记身份

- ID Token 的 `sub` 就是 UnionID（华为文档「ID Token的使用场景与使用方法」的字段表），服务端通用代码本来就取 `sub`，不需要为华为另写取值逻辑。
- UnionID 在同一开发者账号下的所有应用和元服务里相同；OpenID 按应用区分（在 `openid` 声明里）。以后同一开发者名下再上架应用或元服务，同一个华为账号仍落到同一个水杉账号。
- 代价是 UnionID 按定义是开发者维度的标识，应用转让给另一个开发者账号后它会变，已有的华为账号身份认不出来，需要用户用邮箱或手机号登录后重新绑定；目前没有转让计划。UnionID 区分大小写，按原样存。

### 业主要做的配置

- AppGallery Connect：在「开发与服务 > 项目 > 应用 > 常规 > 应用」看应用的 Client ID 和 APP ID。只要 `openid` 不需要申请账号权限（「申请账号权限」只针对一键登录、手机号、收货地址等）；`compatibleSdkVersion` 是 6.0.1(21)，不需要配置公钥指纹（「配置签名和指纹」：compatibleSdkVersion ≥ 20 的应用不需要）。
- 客户端：Client ID 与 APP ID 不同时，在 `platforms/harmony/entry/src/main/module.json5` 的 module 级 `metadata` 写 `{"name": "client_id", "value": "<应用 Client ID>"}`（直接写数字，不能用 `$string` 引用，也不能是项目 Client ID）；相同时不用写。`scripts/test-harmony-manifest.py` 核对这项的位置与形状，占位值（如文档示例的 `xxxxx`）会让检查失败。发布工作流里的 `HARMONY_AGC_CLIENT_ID` 是 Publishing API 的客户端，与这里无关。
- 服务端：先发布带 `huawei` 校验的版本到全部副本，再把同一个 Client ID 写进 `auth.huawei.client_ids` 并滚动重启（旧版本不认识这个字段会拒绝启动）。此后 `/v1/auth/providers` 返回 `huawei: true`，鸿蒙设置应用才显示按钮。

依据的华为文档：`harmonyos-references/account-api-authentication`（授权请求的 `scopes`、`permissions`、`nonce`、`idTokenSignAlgorithm`、错误码）、`harmonyos-guides/account-faq-12`（ID Token 字段、发行方、JWKS 地址、服务端验签示例）、`harmonyos-guides/account-faq-9`（OpenID 与 UnionID）、`harmonyos-guides/account-client-id`、`harmonyos-guides/account-sign-fingerprints`、`harmonyos-guides/account-config-permissions`，以及 `https://oauth-login.cloud.huawei.com/.well-known/openid-configuration`。

## Alternatives considered

- **保留 Google，只加华为账号。** 最强的理由是海外用户和已经用 Google 注册的用户在鸿蒙上还能登录同一个账号。不这样做，是因为鸿蒙版只在中国大陆的华为应用市场上架，那里点 Google 按钮必然失败，审核员正是因此提出意见；一个必定失败的按钮比没有更糟。Google 代码保留，需要时一个开关就能恢复。
- **只留手机号登录，不加第三方登录。** 最强的理由是不用接 Account Kit、不用改服务端、不用业主去 AGC 配置。不这样做，是因为审核意见明确建议提供华为账号登录，而且鸿蒙用户的系统里本来就登着华为账号，一键授权比收验证码少一步。
- **按华为推荐的服务端换码：客户端只传授权码，服务端用 Client Secret 换 Access Token 再查 UnionID。** 最强的理由是华为文档把它当推荐做法，说能防数据遍历、身份伪造和重放。不这样做，是因为服务端要新增 Client Secret 的保管和一套换码请求，而同样的安全性在 ID Token 路径上已经有：签名由华为的 JWKS 校验，受众限定为本应用的 Client ID，nonce 绑定服务端发的一次性 challenge，挡住了重放和伪造。华为的「ID Token的使用场景与使用方法」也把「有服务器的应用在服务端解析与验证 ID Token」列为推荐用法。
- **按 OpenID 记身份。** 最强的理由是 OpenID 只在本应用内有效，不把同一开发者名下其他应用的身份连在一起，泄露面更小。不这样做，是因为 ID Token 的 `sub` 是 UnionID，取 OpenID 要为华为单独读 `openid` 声明；而 UnionID 让同一开发者名下的其他应用、元服务共用身份。
- **把 `client_id` 做成构建时注入的变量，构建缺它就失败。** 最强的理由是不会忘了配。不这样做，是因为 Client ID 与 APP ID 相同时根本不需要这项，构建无从判断该不该失败；华为文档还要求 value 直接写在 `module.json5` 里。仓库对 AGC 签发的标识（`dataGroupIds`）也是直接写进 `module.json5`，这里照做，只在值写错时让检查失败。

## Consequences

- **收益**：鸿蒙用户可以用系统里登着的华为账号一键登录；审核员不会再点到一个打不开的 Google 按钮；服务端只多一条 OIDC 配置，没有新密钥、新表或新权限。
- **代价**：鸿蒙上用 Google 注册过的用户不能再用 Google 登录，要先用邮箱或手机号登录（若账号里绑过）；华为账号身份暂不支持「添加登录方式」的绑定流程。按钮是 ArkWeb 里的普通按钮而不是 SDK 的 `LoginWithHuaweiIDButton`，若审核按华为账号登录按钮视觉规范挑样式，要再调整。
- **时序**：服务端先上线并配置 `auth.huawei.client_ids`，再发带这项改动的鸿蒙版。顺序反过来时鸿蒙版先隐藏了 Google，而华为账号按钮要等服务端开关打开才出现，这段时间只剩邮箱和手机号。

## Verification

- 服务端（msime-cloud）：`TestHuaweiIDTokenVerification` 用本地生成的 RSA 密钥签 PS256 和 RS256 令牌、经替身 JWKS（请求必须发到华为的 JWKS 地址）走真实的 `makeVerifiers`，核对受众、发行方、nonce（含缺省的「default」）、有效期、签发时间、空 `sub`、未列入的算法和别的密钥都被拒，身份是 `sub`（UnionID）不是 `openid`；`TestHuaweiIDTokenLoginHTTP` 在 PostgreSQL 上走 `/v1/auth/providers`、`/v1/auth/challenges`、`/v1/auth/login` 全程，重复登录是同一个账号，已属于别人的华为账号绑定返回 409。
- 共享 Rust：`client-services` 的 `huawei_challenge`、`sign_in_huawei_id_token` 单测与传输测试，页面不能用 `request_code` 申请华为 challenge；`host-api` 经 C 入口对回环假服务端完成 `huawei_challenge` → `huawei_login`，没有 nonce 的 challenge 不交给宿主，形状不对的请求在联网前是 `account_invalid`。
- 共享设置页：`apps/desktop/tests/account/huawei-login.test.tsx` 覆盖鸿蒙宿主形状（有 `huaweiLogin`、没有 `googleLogin`）显示华为按钮、不显示 Google，同意勾选拦截，服务端关闭时不显示，失败提示，以及桌面宿主形状不变。
- 鸿蒙：`tests/huawei-sign-in.test.ts` 与 `tests/account-requests.test.ts`；`hvigorw assembleHap` 编译通过（本机没有编 `libmsimeclient.so`，包不能装机）。
- 没有做的：没有在设备或模拟器上拉起 Account Kit，没有用真实的华为账号 ID Token 走过服务端；业主在 AGC 拿到 Client ID、服务端配置好以后要在真机上验收一次。
