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

/** 会换掉本机登录会话的操作：之前之后都要取消旧账号排队中的词库快照、丢掉快照预览。宿主自己的 Google 回环登录（`google_sign_in`）也算。 */
export const SESSION_CHANGING_OPERATIONS: readonly string[] = [
  "login",
  "logout",
  "account_delete",
  "forget",
  "google_sign_in",
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

/** 页面的账号动作，拼成 `msime_client_account` 的请求。宿主自己的 `google_sign_in`、`google_cancel` 在这之前就处理掉了。 */
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

/** 宿主自己发的账号操作（设置同步、Google 回环、会话镜像），不经页面的名单。`actionDocument` 是宿主拼好的动作原文，读不懂时为 null。 */
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
];

/** 页面的社区动作，拼成 `msime_client_community` 的请求。`keyboard_skin_install` 要把皮肤写进皮肤库和试用，所以带上状态目录。 */
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
  if (operation === "keyboard_skin_install") request.state_root = stateRoot;
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

/** 设置页和键盘的云剪贴板动作，拼成 `msime_client_account_cloud_clipboard` 的请求。`deviceName` 是端到端加密登记本机时给其他设备看的名称。 */
export function cloudClipboardRequest(
  directory: string,
  deviceName: string,
  actionDocument: string,
): RequestPlan {
  const action: JsonObject | null = parseObject(actionDocument);
  const operation: string =
    action !== null && typeof action.operation === "string" ? action.operation : "";
  if (action === null || !PAGE_CLOUD_CLIPBOARD_OPERATIONS.includes(operation)) {
    return refused(operation, "invalid_cloud_clipboard");
  }
  const request: JsonObject = { directory, action };
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
