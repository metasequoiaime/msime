# 设置与主题的 WebDAV 同步

对应 #10787。当前实现覆盖共享 Rust 文档边界、加密事件、独立设备批准、WebDAV 日志读写、持久化调度和宿主 C/JSON 入口，尚未在原生设置页开放，共享后台线程已在 macOS IMK、Linux IBus/Fcitx5 和 Windows 正式 Server 的进程生命周期接线；这些平台仍缺原生连接配置页，移动三端仍缺启停接线。完整范围见[设计](../.agents/notes/proposed/feature/2026-11-04-webdav-settings-theme-sync.md)，已实现的选择见[核心笔记](../.agents/notes/implemented/feature/2026-11-04-webdav-settings-sync-boundary.md)。不能据此宣称六端已支持。

## 数据边界

`client-core::settings_sync` 按显式字段白名单投影已校验的 `Preferences`，不上传未知字段。`settings_sync/fields.rs` 列共享设置，主题字段由 `THEME_FIELDS` 单列。凭据、代理、服务配置、宿主快捷键、应用规则、窗口位置、设备布局尺寸、下载资源和本机皮肤/插件引用保留在本机；自定义主题只传有限颜色和经过现有校验的键盘设计，候选皮肤引用不传。

文档格式是 `app.msime.settings-sync`、版本 1，`shared` 与 `theme` 是字段值，`deleted_shared` 与 `deleted_theme` 是恢复默认的显式墓碑。序列化省略的默认值也产生墓碑；外部旧文档没提供的字段沿用基线，不解释成删除。单文档最多 1 MiB，读取时校验格式、域、类型、未知字段与偏好取值，超限整份拒绝。

首次加入已有同步空间时采用远端设置；之后按本机上一次成功同步的投影做字段级三方合并。不同字段的改动自动合并，同字段冲突返回字段名并保留双方，本轮不写本机偏好也不追加远端事件。当前还没有用户选择冲突版本的入口，完整设计中的逻辑时钟和主题双版本尚待实现。

需要上传时，先独占追加合并事件，再用 `PreferencesStore::save` 的修订号比较交换写回本机；抢号失败不先应用下载，避免下轮把远端变更误记为本机修改。网络等待期间用户的编辑不会被旧快照覆盖；写回因修订冲突失败时保留新编辑，并确认已发布的旧快照，下一轮继续三方合并。文档应用沿用设置文件导入的版本裁剪和一次性迁移标记规则，自定义主题恢复默认也保留本机候选皮肤引用。

## 中断恢复

发布或应用前，先把本轮可移植投影和事件写入私有的 `settings-sync-pending`；文件最多 4 MiB，严格解析并绑定网盘、账号空间与设备身份，不保存整份偏好或凭据。依次确认远端发布、按修订号写回偏好、推进 `settings-sync-state`，最后删除待恢复记录。任一步存储失败即停止，保留可恢复状态。

重启后先持有同一把同步锁，再核对待恢复事件和防回退链头。MOVE 已落地但响应丢失时，以实际密文事件确认那次发布，不重复追加。偏好已写回而基线未提交时，以已应用的目标投影推进基线，不把下载内容重放成本机编辑；已提交基线留下的事务文件只做清理，不回滚后来的编辑。明确未写回的并发编辑继续保留。

如果中断后又有本机编辑，无法区分下载是否已生效，返回 `settings_sync_recovery_conflict`，同时保留本机、远端和待恢复记录，等待显式冲突选择；当前尚无该选择的原生入口。恢复也必须验证已确认的远端事件，发现删除或替换时停止，不能清除待恢复记录绕过防回退。恢复路径会额外读取事件链，后续请求额度需要计入这些请求。

## 远端布局与加密

所有路径都在用户 WebDAV 基地址下的独立目录中：

```text
msime-settings/v1/format.json
msime-settings/v1/e-000000000000.json
msime-settings/v1/e-000000000001.json
msime-settings/v1/tmp/<时间>-<随机数>.json
msime-settings/keyring/v1/...
```

密钥串是现有云剪贴板设备批准协议的独立实例，映射到 `msime-settings/keyring/`；本机目录同样独立，不复用剪贴板或词库的身份、批准或缓存。第一台设备建钥，后续设备经过核对码批准后才能读写设置事件。每轮首先核对远端批准状态，轮内请求可复用现有短缓存。

事件明文格式 `app.msime.settings-sync-event` 包含设备公钥摘要、序号、父明文事件的 SHA-256 和文档。使用独立标签 `msime-settings-v1-key` 派生密钥，XChaCha20-Poly1305 加密，24 字节 nonce 由系统随机源生成。网盘信封 `app.msime.settings-sync-sealed` 的格式、版本、密钥编号、序号、设备、父摘要均纳入 AAD；解密后再次核对头部。密文长度在 base64 解码之前受限，不允许网盘响应诱发无限分配。

追加复用既有 WebDAV 传输：先 PUT 唯一临时文件，再 MOVE 到序号路径，`Overwrite: F` 保证只能一台设备成功。抢号失败返回 `settings_conflict`，下轮重拉重算，绝不覆盖已经存在的事件。已知链头每轮直接读取并对照，不信任列目录结果；回退、缺失、断链或跳号停止同步。

目前最多保留 1024 条事件，一次最多拉取 8 MiB 密文。达到上限返回 `settings_sync_log_full`，不悄悄清理历史。检查点压缩、断点分批拉取、移除设备后的重加密与密钥轮换尚待完成，不能通过删除缓存绕开防回退。

## 宿主入口

C 入口 `msime_client_settings_sync` 接受最多 64 KiB 的 UTF-8 JSON，返回值用 `msime_client_string_free` 释放。Rust 宿主可直接调用 `msime_host_api::settings_webdav::request_json`。请求示例：

```json
{
  "preferences_directory": "/synthetic/private/preferences",
  "config": {
    "url": "https://dav.example.test/dav",
    "username": "synthetic-user",
    "password": "synthetic-app-password"
  },
  "device_name": "合成设备",
  "create_identity": false,
  "action": {"operation": "status"}
}
```

请求可以显式提供尚未保存的连接，也可以省略 `config`，由共享层读取本机 `settings_sync_webdav`；没有配置时返回 `settings_sync_not_configured`。该段与词库、云剪贴板的连接分别保存，应用密码进入本机凭据清单，诊断脱敏、设置导出、导入和 WebDAV 文档都不会迁移它。连接与其他网盘配置一样保存在应用私有偏好中，宿主不把它当作系统钥匙串。恢复默认保留已有连接，明确停止同步时清空三项。目录固定派生为 `preferences_directory/settings-sync`，包含私有状态、独立密钥串、密钥日志缓存和进程间锁；目录不能进入备份或设置导出。

- `configuration`：离线返回偏好 `revision`、`configured`、`url`、`username`、`password_saved`，不返回应用密码，不建目录或身份。
- `configure`：必填 `expected_revision`，用请求的 `config` 原子保存连接；不联网、不登记设备。旧修订返回 `conflict`。三项全空表示停止；非空连接必须三项合规。`keep_password: true` 只在草稿密码为空且地址、账号与已存连接逐字一致时沿用旧密码，不能把旧凭据带到另一个地址。响应与 `configuration` 相同。
- `status`：不联网、不生成身份、不建目录；返回 `device_id`、`code`、`approved`、`joined`、`sequence` 和 `pending_write`。
- `run`：一次联网同步，返回 `revision`、`uploaded`、`downloaded`、`conflicts`。
- `scheduled`：必填 `trigger`，值为 `local_change`、`opened`、`periodic` 或 `manual`；由共享层决定是否联网，返回 `network`、可空的 `report`、`next_check_ms` 和 `status`。执行后的 `report` 使用 `Ok`/`Err` 表示结果，错误仅含稳定错误码；未执行时为 `null`。宿主按 `next_check_ms` 再从工作线程调用，不自己计算退避。
- `schedule_status`：离线返回 `pending_changes`、`last_success_ms`、`last_error` 和 `budget_wait_until`，不建目录或身份。
- `devices`：登记/读取设备，结果与现有云剪贴板的设备列表同形。
- `approve`、`reject`：另带 32 位小写十六进制 `device_id`，结果为设备列表。

`create_identity` 默认 `false`。设置应用在用户首次配置时才能显式传 `true`，输入法进程保持 `false`；无身份时不建目录、不联网，返回 `clipboard_identity_missing`。除 `status` 外所有操作均可能阻塞联网，必须由宿主工作线程执行。

`scheduled` 比较白名单投影摘要，本机专属字段变化不重置一分钟防抖；连续编辑最多等十分钟。打开页面距上次拉取至少一分钟，定时拉取间隔三十分钟。请求额度按本安装的滚动三十分钟窗口计算：自动最多 40 次，总计最多 60 次；每次传输之前私有落盘预扣，包含密钥串、目录操作和失败重试。额度用完会返回等待时间，改网盘配置、进程重启和时钟回拨不会清空已用额度。错误退避也持久化，手动操作可重试但不能绕过总额度。上传时的新编辑在下一轮继续发送，不被旧快照标为已经同步。

旧 `run` 和设备管理入口保留原行为；新增原生自动同步使用 `scheduled`。`msime_client_settings_sync_start` 接受 `{preferences_directory}`，只解析路径并创建后台线程，不在调用线程读文件或联网。线程每五秒检查偏好修订；修订变化调用 `local_change`，其余轮次按服务返回的间隔调用 `periodic`。未配置和未配对的安装不联网、不建同步身份或状态目录。重复启动同一目录无动作，切换目录必须等旧轮次退出；停止超时保留线程句柄，后续还能继续等待，防止宿主卸载前误报已结束。

Windows 的 `SettingsSyncWorker` 只在正式 Server 完成准备、进入消息循环前启动，从已准备的 HostOptions 取偏好目录，不另传凭据。预览实例不启动；正常退出与异常展开都发送停止信号，每次最多等待两秒，未结束的轮次仍由 Rust 持有，不阻塞输入线程或插件卸载。

中断后的显式冲突选择、连接探测、原生设置页的连接编辑与批准界面、Windows、Android、iOS 与 HarmonyOS 的线程启动与停机接线、主题库迁移仍未完成。当前 PR 保持 Draft，不新增 React 设置入口、不修改现有设置默认值。
