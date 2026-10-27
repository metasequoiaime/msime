/**
 * 鸿蒙宿主交给 Rust 账号入口的请求文档（#8201 第 2 步）：会话、刷新令牌和全部 HTTP 都在 Rust 的目录模式里（`msime_client_account`、`msime_client_community`、`msime_client_cloud_dictionary`、`msime_client_account_cloud_clipboard`、`msime_client_ai_skin_generate`），宿主只决定把哪个目录、哪个 User-Agent 交给它，以及页面能发哪些操作。
 *
 * 页面发来的动作按操作名放行：只放共享页面在本宿主上已有的操作，其余的（包括带文件路径的 `upload_avatar`、`data_export`、`export_file`、`snapshot_*_file`、`preferences_upload` 这类）一律按各入口的「请求不合法」码拒绝。带路径的字段只由 ArkTS 经 DocumentViewPicker 选好、复制进私有目录后填写；页面能写的字段由 Rust 的 `deny_unknown_fields` 再查一遍，所以放行的操作原样转交，`feedback` 例外，宿主按字段重建，丢掉页面不该给的 `screenshots` 并填上本平台。
 *
 * 这里只拼文档、读回复，不碰 NAPI 和文件，可以在 node 下测试；ArkTS 一侧只传字符串进出。
 */

type JsonObject = Record<string, unknown>;

function jsonObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function parseObject(text: string): JsonObject | null {
  try {
    const value: unknown = JSON.parse(text);
    return jsonObject(value) ? value : null;
  } catch {
    return null;
  }
}

function failure(code: string): string {
  return JSON.stringify({ ok: false, error: code });
}

/** 一次请求的结果：`request` 是交给 NAPI 的文档；不放行时 `request` 为空串，`refusal` 是直接交回页面的回复。 */
export interface RequestPlan {
  readonly operation: string;
  readonly request: string;
  readonly refusal: string;
}

function refused(operation: string, code: string): RequestPlan {
  return { operation, request: "", refusal: failure(code) };
}

function planned(operation: string, request: JsonObject): RequestPlan {
  return { operation, request: JSON.stringify(request), refusal: "" };
}

/** 页面经「账号」通道能发的操作，名字与 `msime_client_account` 相同。 */
export const PAGE_ACCOUNT_OPERATIONS: readonly string[] = [
  "status",
  "providers",
  "request_code",
  "login",
  "profile",
  "rename",
  "avatar",
  "logout",
  "account_delete",
  "forget",
  "qq_links",
  "qq_link_redeem",
  "qq_link_delete",
  "chat_models",
  "chat",
  "feedback",
];

/** 会换掉本机登录会话的操作：之前之后都要取消旧账号排队中的词库快照、丢掉快照预览。宿主自己的 Google 回环登录（`google_sign_in`）和华为账号登录（`huawei_sign_in`）也算。 */
export const SESSION_CHANGING_OPERATIONS: readonly string[] = [
  "login",
  "logout",
  "account_delete",
  "forget",
  "google_sign_in",
  "huawei_sign_in",
];

/** 宿主自报的 User-Agent 只在不为空时带上；`msime_client_client_agent` 失败时宿主得到空串，请求仍以默认 User-Agent 发出。 */
function withAgent(request: JsonObject, clientAgent: string): JsonObject {
  if (clientAgent.length > 0) request.client_agent = clientAgent;
  return request;
}

function feedbackAction(action: JsonObject): JsonObject {
  const rebuilt: JsonObject = {
    operation: "feedback",
    type: action.type,
    text: action.text,
    platform: "harmony",
    app_version: action.app_version,
    edition: action.edition,
  };
  // 页面在没有诊断信息时给 null；Rust 只收字符串到字符串的对象。
  if (jsonObject(action.diagnostics)) rebuilt.diagnostics = action.diagnostics;
  return rebuilt;
}

/** 页面的账号动作，拼成 `msime_client_account` 的请求。宿主自己的 `google_sign_in`、`google_cancel`、`huawei_sign_in` 在这之前就处理掉了。 */
export function accountRequest(
  directory: string,
  clientAgent: string,
  actionDocument: string,
): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null || !PAGE_ACCOUNT_OPERATIONS.includes(operation)) {
    return refused(operation, "account_invalid");
  }
  const forwarded: JsonObject = operation === "feedback" ? feedbackAction(action) : action;
  return planned(operation, withAgent({ directory, action: forwarded }, clientAgent));
}

/** 宿主自己发的账号操作（设置同步、Google 回环、华为账号登录、会话镜像），不经页面的名单。`actionDocument` 是宿主拼好的动作原文，读不懂时为 null。 */
export function hostAccountRequest(
  directory: string,
  clientAgent: string,
  actionDocument: string,
): string | null {
  const action: JsonObject | null = parseObject(actionDocument);
  if (action === null || typeof action.operation !== "string") return null;
  return JSON.stringify(withAgent({ directory, action }, clientAgent));
}

/** 页面经社区通道能发的操作，名字与 `msime_client_community` 相同。`finish_trial`、`store_reply`、`remove_reply` 不联网，由宿主在这之前处理。 */
export const PAGE_COMMUNITY_OPERATIONS: readonly string[] = [
  "keyboard_skin_list",
  "keyboard_skin_detail",
  "keyboard_skin_install",
  "keyboard_skin_rate",
  "keyboard_skin_publish",
  "keyboard_skin_set_category",
  "keyboard_skin_unpublish",
  "report",
  "resource_list",
  "resource_detail",
  "resource_publish",
  "resource_apply",
  "resource_save",
  "resource_rate",
  "resource_unpublish",
  "plugin_list",
  "plugin_detail",
  "plugin_pack_preview",
  "plugin_publish",
  "plugin_install",
  "plugin_rate",
  "plugin_delete",
];

/** 要读写插件目录（`<state_root>/plugins`）的社区插件操作：打包预检、发布和安装。 */
const PLUGIN_ROOT_OPERATIONS: readonly string[] = [
  "plugin_pack_preview",
  "plugin_publish",
  "plugin_install",
];

/** 页面的社区动作，拼成 `msime_client_community` 的请求。`keyboard_skin_install` 要把皮肤写进皮肤库和试用，插件的打包预检、发布和安装要读写插件目录，所以带上状态目录。 */
export function communityRequest(
  directory: string,
  stateRoot: string,
  actionDocument: string,
): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null || !PAGE_COMMUNITY_OPERATIONS.includes(operation)) {
    return refused(operation, "community_invalid");
  }
  const request: JsonObject = { directory, action };
  if (operation === "keyboard_skin_install" || PLUGIN_ROOT_OPERATIONS.includes(operation)) {
    request.state_root = stateRoot;
  }
  return planned(operation, request);
}

/** 页面经云词库通道能发的短请求，名字与 `msime_client_cloud_dictionary` 相同。`export` 和 `snapshot_*` 要经文件选择器，由宿主自己拼。 */
export const PAGE_CLOUD_DICTIONARY_OPERATIONS: readonly string[] = [
  "list",
  "catalog",
  "add",
  "update",
  "delete",
  "edit_catalog",
  "candidates",
  "rank",
  "remove_candidate",
  "fixed_positions",
  "set_fixed_position",
  "import",
];

export function cloudDictionaryRequest(directory: string, actionDocument: string): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null || !PAGE_CLOUD_DICTIONARY_OPERATIONS.includes(operation)) {
    return refused(operation, "invalid_cloud_dictionary");
  }
  return planned(operation, { directory, action });
}

/** 宿主自己拼的云词库请求：整份快照和导出文件。`options` 是 `prepareHost` 给的 HostOptions 文档原文，`snapshot_directory` 是设置应用私有的中转目录。 */
export function cloudDictionaryFileRequest(
  directory: string,
  snapshotDirectory: string,
  optionsDocument: string | null,
  actionDocument: string,
): string | null {
  const action: JsonObject | null = parseObject(actionDocument);
  if (action === null || typeof action.operation !== "string") return null;
  const request: JsonObject = { directory, snapshot_directory: snapshotDirectory, action };
  if (optionsDocument !== null) {
    const options: JsonObject | null = parseObject(optionsDocument);
    if (options !== null) request.options = options;
  }
  return JSON.stringify(request);
}

/** 共享云剪贴板面板的动作。置顶、保留期和清空 Rust 也认，但共享页面在本宿主上还没有入口，不放行。 */
export const PAGE_CLOUD_CLIPBOARD_OPERATIONS: readonly string[] = [
  "list",
  "add",
  "delete",
  "set_enabled",
  "devices",
  "approve",
  "reject",
  "remove",
  "reset",
];

/** 页面或键盘发来的云剪贴板动作，按名单放行；不放行时为 null。 */
function cloudClipboardAction(actionDocument: string): JsonObject | null {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: unknown = action === null ? undefined : action.operation;
  return action !== null &&
    typeof operation === "string" &&
    PAGE_CLOUD_CLIPBOARD_OPERATIONS.includes(operation)
    ? action
    : null;
}

function operationName(actionDocument: string): string {
  const action: JsonObject | null = parseObject(actionDocument);
  return action !== null && typeof action.operation === "string" ? action.operation : "";
}

/**
 * 设置页和键盘的云剪贴板动作，拼成 `msime_client_account_cloud_clipboard` 的请求。`deviceName` 是端到端加密登记本机时给其他设备看的名称。
 *
 * `preferencesDirectory` 总是带上：Rust 在发账号请求之前再读一次偏好，已经改成自建中继或 WebDAV 网盘时答 `clipboard_source_changed`，宿主先问的来源过期了也不会把内容发到水杉账号。`createIdentity` 为假时只用本机已有的设备身份（键盘扩展，见 `cloudClipboardRoute`）。
 */
export function cloudClipboardRequest(
  directory: string,
  deviceName: string,
  actionDocument: string,
  preferencesDirectory: string,
  createIdentity: boolean,
): RequestPlan {
  const action: JsonObject | null = cloudClipboardAction(actionDocument);
  if (action === null) return refused(operationName(actionDocument), "invalid_cloud_clipboard");
  const request: JsonObject = { directory, preferences_directory: preferencesDirectory, action };
  if (deviceName.length > 0) request.device_name = deviceName;
  if (!createIdentity) request.create_identity = false;
  return planned(String(action.operation), request);
}

/** `msime_client_cloud_clipboard_source` 的回答：走水杉账号，走自建中继或 WebDAV 网盘，或者读不出来（`error` 是交给页面的码，例如偏好读不了的 `storage`、配置不合规的 `relay_not_configured`）。 */
export interface CloudClipboardRoute {
  readonly source: "account" | "relay" | "failed";
  /** 中继的后端：`relay` 或 `webdav`；不是中继时为空串。 */
  readonly backend: string;
  /** 读出来源时偏好的修订号，不含凭据；读不出时为 -1。 */
  readonly revision: number;
  readonly error: string;
}

function routeFailure(code: string): CloudClipboardRoute {
  return { source: "failed", backend: "", revision: -1, error: code };
}

/**
 * 读 `msime_client_cloud_clipboard_source` 的回答。读不懂的一律按失败处理，从不当成「走账号」：配置了中继的用户以为内容只去自己的中继，退回账号就是把它发给了水杉服务。
 */
export function cloudClipboardRoute(reply: string): CloudClipboardRoute {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null) return routeFailure("storage");
  if (envelope.ok !== true) {
    return routeFailure(typeof envelope.error === "string" ? envelope.error : "storage");
  }
  const value: unknown = envelope.value;
  if (!jsonObject(value)) return routeFailure("storage");
  const revision: unknown = value.revision;
  if (typeof revision !== "number" || !Number.isSafeInteger(revision) || revision < 0) {
    return routeFailure("storage");
  }
  if (value.source === "account") return { source: "account", backend: "", revision, error: "" };
  if (value.source === "relay" && (value.backend === "relay" || value.backend === "webdav")) {
    return { source: "relay", backend: value.backend, revision, error: "" };
  }
  return routeFailure("storage");
}

/**
 * 键盘的云剪贴板列表属于哪条路：中继或网盘是 `relay:<后端>:<偏好修订号>`，不含地址和凭据。账号路径仍按登录的账号 id 绑定（`statusUserId`），两种写法不会相同。偏好的任何改动都会让中继的绑定失效，代价是键盘里改了别的设置之后要重新打开一次云端列表；换来的是绑定不必带凭据，Rust 还按同一个修订号在读配置的那一刻再比较一次（`expected_revision`）。
 */
export function relayBinding(route: CloudClipboardRoute): string {
  return route.source === "relay" ? `relay:${route.backend}:${route.revision}` : "";
}

/** `relayBinding` 里的偏好修订号；不是中继的绑定时为 -1。 */
export function relayBindingRevision(binding: string): number {
  const match: RegExpExecArray | null = /^relay:(relay|webdav):(\d{1,15})$/.exec(binding);
  return match === null ? -1 : Number(match[2]);
}

/**
 * 设置页和键盘的云剪贴板动作，拼成 `msime_client_relay_cloud_clipboard` 的请求，经偏好里配置的自建中继或 WebDAV 网盘答。动作名单与账号路径相同：置顶、保留期和清空 Rust 也认，但共享页面在本宿主上还没有入口。
 *
 * `directory` 放中继的密钥串、网盘日志的本机缓存和锁文件，两个沙箱各用自己的 `files/state`。`expectedRevision` 是刚读来源时的偏好修订号，Rust 读配置时不符就答 `clipboard_source_changed`、不联网。`createIdentity` 为假时只用 `directory` 里已有的密钥串：键盘扩展不自己生成设备身份，只用设置应用经共享沙箱镜像过来的那份，同一台设备只登记一次。
 */
export function relayCloudClipboardRequest(
  preferencesDirectory: string,
  directory: string,
  deviceName: string,
  actionDocument: string,
  expectedRevision: number,
  createIdentity: boolean,
): RequestPlan {
  const action: JsonObject | null = cloudClipboardAction(actionDocument);
  if (action === null) return refused(operationName(actionDocument), "invalid_cloud_clipboard");
  const request: JsonObject = {
    preferences_directory: preferencesDirectory,
    directory,
    action,
  };
  if (Number.isSafeInteger(expectedRevision) && expectedRevision >= 0) {
    request.expected_revision = expectedRevision;
  }
  if (deviceName.length > 0) request.device_name = deviceName;
  if (!createIdentity) request.create_identity = false;
  return planned(String(action.operation), request);
}

/** WebDAV 设置的「测试连接」：页面交来的草稿 `{url, username, password}`，拼成 `msime_client_cloud_clipboard_webdav_probe` 的请求。地址和凭据的规则由 Rust 校验，这里只认形状；`directory` 是本机中继密钥串所在的目录，通过后 Rust 删掉那里的网盘日志缓存。 */
export function webDavProbeRequest(draftDocument: string, directory: string): RequestPlan {
  const draft: JsonObject | null = parseObject(draftDocument);
  if (
    draft === null ||
    Object.keys(draft).some(
      (key: string): boolean => !["url", "username", "password"].includes(key),
    ) ||
    typeof draft.url !== "string" ||
    typeof draft.username !== "string" ||
    typeof draft.password !== "string"
  ) {
    return refused("probe", "relay_not_configured");
  }
  return planned("probe", {
    url: draft.url,
    username: draft.username,
    password: draft.password,
    directory,
  });
}

/** 页面经「词库同步」通道能发的动作，名字与 `msime_client_dictionary_sync` 相同。 */
export const PAGE_DICTIONARY_SYNC_OPERATIONS: readonly string[] = [
  "run",
  "status",
  "devices",
  "approve",
  "reject",
  "remove",
  "reset",
  "restart",
  "probe",
];

/** 本机词库同步状态的目录：设置应用沙箱 `files/state` 下的 `dictionary-sync`。不直接用 `files/state`：云剪贴板的中继在那里放着 `cloud-clipboard-relay-keystore` 和 `cloud-clipboard-webdav-state`，词库的密钥串用同样的文件名，放在一起会互相覆盖。 */
export function dictionarySyncDirectory(stateDirectory: string): string {
  return `${stateDirectory}/dictionary-sync`;
}

/**
 * 「我的词条」经 WebDAV 网盘同步的一个动作，拼成 `msime_client_dictionary_sync` 的请求。`optionsDocument` 是 `prepareHost` 给的 HostOptions 文档（带偏好目录，Engine 的词库路径也从它来），`stateDirectory` 是设置应用的 `files/state`。动作按操作名放行，字段由 Rust 的 `deny_unknown_fields` 再查一遍。
 *
 * 只有设置应用发这个请求。键盘扩展在自己的沙箱里，读写的是它自己那份个人词库，这个入口又没有「只用已有身份」的开关，键盘一调就会在网盘上登记成另一台设备，所以键盘不调它（`scripts/test-harmony-dictionary-sync.py` 守着）。
 */
export function dictionarySyncRequest(
  optionsDocument: string,
  stateDirectory: string,
  deviceName: string,
  actionDocument: string,
): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const options: JsonObject | null = parseObject(optionsDocument);
  const operation: unknown = action === null ? undefined : action.operation;
  if (
    action === null ||
    options === null ||
    typeof operation !== "string" ||
    !PAGE_DICTIONARY_SYNC_OPERATIONS.includes(operation)
  ) {
    return refused(operationName(actionDocument), "invalid_dictionary_sync");
  }
  const request: JsonObject = {
    options,
    directory: dictionarySyncDirectory(stateDirectory),
    action,
  };
  if (deviceName.length > 0) request.device_name = deviceName;
  return planned(operation, request);
}

/**
 * 设备在端到端加密里登记的名称：`marketName`（如「HUAWEI Mate 70」），没有时用 `productModel`，两者都没有时用「HarmonyOS 设备」，与原来的 ArkTS 桥接相同。总要带上，否则 Rust 退回读主机名，鸿蒙上会显示成「Linux」。
 */
export function cloudClipboardDeviceName(marketName: string, productModel: string): string {
  const market: string = marketName.trim();
  if (market.length > 0) return market;
  const model: string = productModel.trim();
  return model.length > 0 ? model : "HarmonyOS 设备";
}

/** `msime_client_client_agent` 的请求。`osRelease` 是 `OsVersionPolicy.release` 读出的版本号，读不出时为 null，系统一栏只写 HarmonyOS。 */
export function clientAgentRequest(
  version: string,
  marketName: string,
  productModel: string,
  osRelease: string | null,
  edition: string,
): string {
  const market: string = marketName.trim();
  return JSON.stringify({
    platform: "harmony",
    version,
    device: market.length > 0 ? market : productModel.trim(),
    system: osRelease === null ? "HarmonyOS" : `HarmonyOS ${osRelease}`,
    edition,
  });
}

/** `msime_client_ensure_anonymous_account_with_agent` 的请求。 */
export function anonymousAccountRequest(directory: string, clientAgent: string): string {
  return JSON.stringify(withAgent({ directory }, clientAgent));
}

/** 页面的「上传到云端」：`{operation:"status"|"set_enabled"|"sync",enabled?}`，换成 Rust 的 `typing_upload_*`。统计目录、本地日期和平台由宿主填。 */
export function typingUploadRequest(
  directory: string,
  clientAgent: string,
  statisticsDirectory: string,
  day: string,
  actionDocument: string,
): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null) return refused(operation, "account_invalid");
  if (operation === "status") {
    return planned(
      operation,
      withAgent(
        {
          directory,
          action: { operation: "typing_upload_status", statistics_directory: statisticsDirectory },
        },
        clientAgent,
      ),
    );
  }
  if (operation === "set_enabled" && typeof action.enabled === "boolean") {
    return planned(
      operation,
      withAgent(
        {
          directory,
          action: {
            operation: "typing_upload_set",
            statistics_directory: statisticsDirectory,
            enabled: action.enabled,
            day,
            platform: "harmony",
          },
        },
        clientAgent,
      ),
    );
  }
  if (operation === "sync") {
    return planned(
      operation,
      withAgent(
        {
          directory,
          action: {
            operation: "typing_upload",
            statistics_directory: statisticsDirectory,
            day,
            platform: "harmony",
          },
        },
        clientAgent,
      ),
    );
  }
  return refused(operation, "account_invalid");
}

/** Rust 的 `{enabled,last_upload_ms,signed_in}` 换成页面读的 `{enabled,lastUploadMs,signedIn}`；失败原样交回。 */
export function typingUploadReply(reply: string): string {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null) return failure("account_unavailable");
  if (envelope.ok !== true) return reply;
  const value: unknown = envelope.value;
  if (
    !jsonObject(value) ||
    typeof value.enabled !== "boolean" ||
    typeof value.signed_in !== "boolean"
  ) {
    return failure("account_unavailable");
  }
  const last: unknown = value.last_upload_ms;
  return JSON.stringify({
    ok: true,
    value: {
      enabled: value.enabled,
      lastUploadMs: typeof last === "number" && Number.isSafeInteger(last) ? last : null,
      signedIn: value.signed_in,
    },
  });
}

/** 页面的 AI 皮肤动作：`generate` 换成 `msime_client_ai_skin_generate` 的请求，`cancel` 换成 `msime_client_ai_skin_cancel` 的请求。 */
export function aiSkinRequest(directory: string, actionDocument: string): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null || typeof action.request_id !== "string") {
    return refused(operation, "ai_skin_invalid");
  }
  if (operation === "generate" && typeof action.prompt === "string") {
    return planned(operation, { directory, request_id: action.request_id, prompt: action.prompt });
  }
  if (operation === "cancel") return planned(operation, { request_id: action.request_id });
  return refused(operation, "ai_skin_invalid");
}

/** 回复成功时的 value，原样序列化；失败或读不懂时为 null。 */
export function replyValueText(reply: string): string | null {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null || envelope.ok !== true || !("value" in envelope)) return null;
  return JSON.stringify(envelope.value);
}

/** 回复的错误码；成功时为空串，读不懂时是 `fallback`。 */
export function replyError(reply: string, fallback: string): string {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null) return fallback;
  if (envelope.ok === true) return "";
  return typeof envelope.error === "string" && envelope.error.length > 0
    ? envelope.error
    : fallback;
}

/** 目录模式 `status` 的回复里登录账号的 id；没有登录或读不了时为 null。 */
export function statusUserId(reply: string): string | null {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null || envelope.ok !== true || !jsonObject(envelope.value)) return null;
  const user: unknown = envelope.value.user;
  return jsonObject(user) && typeof user.id === "string" && user.id.length > 0 ? user.id : null;
}

/** 设置同步上传的第一步：`msime_client_account_harmony_settings_export` 的请求。`feedback`、`schema`、`cloud` 都是 JSON 原文。 */
export function settingsExportRequest(
  preferencesDirectory: string,
  feedbackDocument: string,
  schemaDocument: string,
  cloudDocument: string,
): string | null {
  const feedback: JsonObject | null = parseObject(feedbackDocument);
  const schema: JsonObject | null = parseObject(schemaDocument);
  const cloud: JsonObject | null = parseObject(cloudDocument);
  if (feedback === null || schema === null || cloud === null) return null;
  return JSON.stringify({ preferences_directory: preferencesDirectory, feedback, schema, cloud });
}

/** 导出给的 `merged`（带着云端 revision 的整份文档）换成 `preferences_put`，绑定读文档时的账号。 */
export function preferencesPutAction(exportValueDocument: string, userId: string): string | null {
  const value: JsonObject | null = parseObject(exportValueDocument);
  if (value === null || !jsonObject(value.merged)) return null;
  const merged: JsonObject = value.merged;
  if (!jsonObject(merged.settings) || typeof merged.revision !== "number") return null;
  return JSON.stringify({
    operation: "preferences_put",
    document: merged.settings,
    revision: merged.revision,
    user_id: userId,
  });
}

/** 设置同步下载：`msime_client_account_harmony_settings_apply` 的请求，总是带按键反馈（文档和字段表都有这三个键时不带是 `account_invalid`）。 */
export function settingsApplyRequest(
  preferencesDirectory: string,
  cloudDocument: string,
  schemaDocument: string,
  feedbackDocument: string,
): string | null {
  const cloud: JsonObject | null = parseObject(cloudDocument);
  const schema: JsonObject | null = parseObject(schemaDocument);
  const feedback: JsonObject | null = parseObject(feedbackDocument);
  if (cloud === null || schema === null || feedback === null) return null;
  return JSON.stringify({ preferences_directory: preferencesDirectory, cloud, schema, feedback });
}

/** 应用以后要写回反馈文件的按键反馈，原样序列化；不用写（null）或读不懂时为 null。 */
export function appliedFeedbackText(reply: string): string | null {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null || envelope.ok !== true || !jsonObject(envelope.value)) return null;
  const feedback: unknown = envelope.value.feedback;
  return jsonObject(feedback) ? JSON.stringify(feedback) : null;
}
