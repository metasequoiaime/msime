# Agent Note: 输入习惯同步的密钥驱动与剪贴板共用原语、不共用状态机

Status: implemented

## Problem

#10043 的第 1 期要在客户端实现 learning 域：本机密钥串、带认证的封装、反向核对码、MSL1 信封和 Padmé 补齐，以及管理设备的 sans-IO 驱动。[定案笔记](../../proposed/feature/2026-10-27-learning-sync-e2e.md)写的是「`clipboard_e2e.rs` 里写死的 `API` 路径和密钥串名改成域参数」，前提是两个域的服务端接口只差路径。msime-cloud #1599 落地后，learning 域的接口和剪贴板已经不只差路径：建钥和换钥分成 `POST /key`、`POST /rekey`（剪贴板共用 `POST /keys`，请求体是 `wraps` 数组），批准要带 `approver_device_id`，状态是 `GET /learning?device_id=`（剪贴板是 `GET /clipboard/keys?device_id=`），还多了每台设备一份的密文和「核对反向核对码」这一步。照原话把剪贴板驱动按域参数化，每个阶段都要按域分支。

## Decision

`crates/client-core/src/cloud/learning_e2e.rs` 是 learning 域自己的一套：密钥串、封装、反向核对码、信封和驱动状态机。它与 `clipboard_e2e.rs` 共用的只是与域无关的部分，剪贴板把这些改成 `pub(crate)` 或原样导出，行为不变：

- 设备编号 `device_id`、核对码的显示形式 `grouped_digits`（剪贴板的 `verification_code` 改为调它，输出逐字节相同，由剪贴板已有的已知答案测试兜住）。
- `random`、`hkdf32`、`decode_exact`、`sanitize_device_name`。
- 宿主看到的请求与响应形状 `HttpRequest`、`HttpResponse`、`DriverError`、`Persisted`、`RandomUnavailable`。

C ABI `msime_client_cloud_clipboard_e2e` 改为调 `cloud::e2e_drive_json`，按请求顶层的 `domain` 分派：没有 `domain` 时把原始字节原样交给剪贴板驱动，`"clipboard"` 同样交给剪贴板，`"learning"` 交给 learning 驱动，其他值报错。JNI 和 NAPI 只是转发这个 C 函数的 JSON，所以三处都有了 `domain` 字段，已有宿主的请求和结果不变。

驱动只管密钥与设备（`status`、`approve`、`reject`、`remove`、`reset`、`confirm`、`deny`、`disable`），不管密文上传下载。同步那一轮用 `Keystore::seal_blob` 和 `Keystore::open_blob`：二进制正文不适合塞进驱动 JSON 的 `body` 文本，而防回滚的 `verified`、`applied` 和合并都属于 client-services 的同步轮次（定案的 PR 6）。

密钥串除了密钥本身，还记着批准者公钥和核对状态（`confirmed`、`unconfirmed`、`rejected`），格式写在 `docs/learning-sync.md` 的「本机的密钥串」。几条定案没写细、这里定下的：

- 用户点「不一致」后，服务端没有删除封装的接口，下一次取状态还会拿到同一份封装。密钥串只记下被拒绝的 `key_id` 和批准者（密钥一栏写 `-`），同一个批准者给同一把密钥的封装不再解开、不再弹核对；换了批准者或换了钥就重新核对。这样用户误点「不一致」后，在另一台设备上重新允许就能恢复。
- 封装的批准者是本机（第一台设备、本机换的钥）时直接采用：带认证的封装里 `dh2 = DH(s_D, P_D)`，只有持有本机私钥的一方算得出来。
- 解压后的明文流上限 128 MiB，解密方读到上限加一个字节就停，上传方也不写出更大的流。8 MiB 的信封按调研的约 1/9 压缩比约装 72 MiB 明文。
- `learning_key_rotated` 只在发现换钥的那一次回报：发现后本机丢掉旧密钥，之后回报普通的等待批准。保留旧密钥只为了让提示一直显示，换来的是一把不能用、却会被 `seal_blob` 拿去加密的密钥，不值得。

## Alternatives considered

- **照定案把剪贴板驱动按域参数化** — 只有一份状态机，以后两个域的改动不会漏改一边。但两个接口的建钥、换钥、批准请求体和状态流转都不同，learning 域还多了反向核对，参数化之后剪贴板的每个阶段都要按域分支；剪贴板已经在六个平台上线，为了新功能改动它的状态机，风险落在已发布的功能上。逐字节不变的回归测试能兜住请求，兜不住分支里的状态流转。
- **把 `run` 和 `Step` 做成按状态类型泛型、两个域共用** — 少写约五十行循环。但 `Step` 和 `DriverState` 是剪贴板模块的公开类型，泛型化改的是已发布驱动的公开接口，还要给两个状态机抽一个 trait，省下的只是约五十行完全相同的循环；以后改一边时对照另一边即可。
- **新开一个 C ABI 函数给 learning 域** — 分派更显式。但 JNI、NAPI 和 Apple 的闭包都要再接一个函数，Android 的 `check-host.sh` 还限制 Java 里拼云端路径的位置；加 `domain` 字段时宿主只改请求 JSON。

## Consequences

- **收益**：剪贴板的请求、请求体和密钥串编码不变（剪贴板的 28 个测试和 host-api 的边界测试原样通过）；learning 域的密码学格式由独立实现算出的已知答案钉住；宿主接 learning 域只需在请求里加 `domain`。
- **代价与已知上限**：两份 `run` 循环要一起维护。设备核对码和反向核对码的强度仍按定案（反向核对码带 `K_learn`，设备核对码约 2⁵³ 的现场第二原像），不防的那几条见协议文档的「威胁模型」。驱动的 `domain` 字段在任何平台接入之前就已经可用；平台在定案的 PR 7 到 10 里各自接入，能力为 false 时不渲染入口。

## Verification

- `cargo test -p msime-client-core --lib cloud::learning_e2e`：已知答案（由 pyca/cryptography 加手写 HChaCha20 算出，先复现了剪贴板已发布的密文）、篡改附加数据里的 `key_id`、`device_id`、`generation` 时解不开、伪造批准者时反向核对码对不上、补齐和 gzip 的边界、按 msime-cloud 语义写的内存服务端上的设备流程。
- `cargo test -p msime-client-core --lib cloud::clipboard_e2e` 与 `cargo test -p msime-host-api --lib cloud_clipboard`：剪贴板行为不变，C ABI 按 `domain` 分派。
- 相关：[云剪贴板端到端加密](../feature/2026-10-11-cloud-clipboard-e2e.md)（被复用的原语的出处）。
