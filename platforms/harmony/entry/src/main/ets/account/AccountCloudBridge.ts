import { utf8Length } from "../keyboard/Utf8";
import { CustomKeyboardSkin, CustomSkinDocument } from "../keyboard/skin/CustomKeyboardSkin";
import { CloudClipboardPolicy } from "../keyboard/clipboard/CloudClipboardPolicy";
import { TextPolicy } from "../keyboard/TextPolicy";

export type AccountTransportResponse = { status: number; body: string; contentLength?: number };
export type AccountDownloadResponse = {
  status: number;
  bytes: number;
  contentLength?: number;
  contentType?: string;
};

export interface AccountTransport {
  /**
   * `timeoutMs` is the read timeout for this one request, not a bridge-wide setting.
   *
   * Every other call here answers from a database and is done in seconds; a chat completion is a
   * model generating text and the shared clients allow it 125. Giving the whole transport that
   * budget would mean a dead network takes two minutes to report on a profile fetch too.
   */
  request(
    method: string,
    path: string,
    token?: string,
    body?: Record<string, unknown>,
    timeoutMs?: number,
    requestTag?: string,
  ): Promise<AccountTransportResponse>;
  cancelRequests?(requestTag: string): void;
  cancelDownloads?(): void;
  download?(
    path: string,
    token: string,
    destination: string,
    maximumBytes: number,
    mediaType: string,
  ): Promise<AccountDownloadResponse>;
  uploadSnapshot?(
    source: string,
    revision: number,
    expectedSha256: string,
    token: string,
  ): Promise<AccountTransportResponse>;
}

export interface AccountSessionStore {
  load(): string | null;
  save(value: string): void;
  clear(): void;
  /** Synchronously cancel work owned by a valid saved session before removing it. */
  beforeClear?(accountId: string): void;
  /**
   * Runs `body` holding a lock that every process sharing this store takes before it refreshes.
   *
   * A store only one process ever reads may leave it out. One that the settings application and the keyboard extension both read must provide it, because the service revokes the whole session when a spent refresh token is presented.
   */
  exclusive?<T>(body: () => Promise<T>): Promise<T>;
}

/**
 * 云剪贴板端到端加密里宿主负责的那一半：原生驱动、本机密钥串和设备名。
 *
 * 流程逻辑只在 Rust 驱动里（`msime_client_cloud_clipboard_e2e`，协议见 `docs/cloud-clipboard-e2e.md`）；本桥接只负责带着账号凭据把驱动要的请求发出去、把响应交回，并在每一步保存驱动交回的密钥串。设置应用和键盘读写同一份密钥串，否则同一台设备会在服务端登记成几台。
 */
export interface CloudClipboardE2eHost {
  /** 调用原生驱动：传入驱动的输入文档，返回 `{ok:true,value}` 或 `{ok:false,error}`。 */
  drive(request: string): string;
  /** 本机保存的密钥串，没有时为 null。 */
  loadKeystore(): string | null;
  /** 原子替换已保存的密钥串；写不进去时抛出。 */
  saveKeystore(value: string): void;
  /** 删除密钥串；退出登录时与会话一起删除。 */
  clearKeystore(): void;
  /** 登记到服务端、在其他设备的批准列表里显示的设备名。 */
  deviceName(): string;
}

type Session = {
  access_token: string;
  refresh_token: string;
  token_type: "Bearer";
  expires_at: number;
  user: { id: string; display_name: string; created_at: string };
};

type Action = Record<string, unknown>;

type CredentialReply = { token?: string; error?: string };
type AuthorizedReply = { response?: AccountTransportResponse; token?: string; error?: string };

/**
 * The envelope ceiling, which is a first line rather than the real bound.
 *
 * Every operation checks its own payload — the clipboard 4,000 characters, a dictionary import 64
 * KB, a chat request 64 KB — so this exists to stop something absurd before it is even parsed. It
 * is 512 KB rather than 64 because publishing a community dictionary carries up to 128 entries of
 * up to 1,024 characters each, and the shared service allows 350,000 bytes of content: a 64 KB
 * envelope would have refused a resource the server would have accepted, on this host only.
 */
const MAX_ACTION_BYTES = 512 * 1024;
/** JSON responses use the same one-megabyte envelope as the shared account client. */
const MAX_JSON_RESPONSE_BYTES = 1024 * 1024;
/** Resource pages and details carry published dictionary content, so use their shared limits. */
const MAX_COMMUNITY_RESOURCE_PAGE_BYTES = 48 * 1024 * 1024;
const MAX_COMMUNITY_RESOURCE_DETAIL_BYTES = 3 * 1024 * 1024;
const MAX_SESSION_SECONDS = 86_400 * 30;
const MAX_SESSION_MILLISECONDS = MAX_SESSION_SECONDS * 1000;
const MAX_SESSION_BYTES = 64 * 1024;
/** Clipboard searches use the shared account client's 1,024-byte query bound. */
const MAX_SEARCH = 1024;
/**
 * 交回驱动的一个云剪贴板响应正文的上限。一页最多 50 条密文，每条不超过 16,384 个字符，加上字段约 830 KB；驱动的输入整体限 2 MiB，还要装下驱动状态和 JSON 转义，所以取 1.5 MiB。
 */
const MAX_CLIPBOARD_RESPONSE_BYTES = 1536 * 1024;
/** 一次云剪贴板操作最多往返几次。驱动自己把取密钥状态限在 4 次，正常操作不超过 10 步；这里兜住驱动出错时的死循环。 */
const MAX_CLIPBOARD_E2E_STEPS = 24;
/** Quick phrase values use the shared dictionary import limit (UTF-16 code units). */
const MAX_QUICK_PHRASE_UTF16 = 199;

/**
 * 对话的各项上限，用的是共享的那一套，而不是 HarmonyOS 自己对它们的解读。
 *
 * 它们与 Apple 上的 `BackendChatClient` 和 `client-services` 的账号客户端逐字节一致，因为服务端执行的是同样的数值，客户端如果更严格，就会把一段能正常进行的对话变成只在某一个平台上出现、又无从解释的拒绝。
 */
const MAX_CHAT_MODELS = 33;
const MAX_CHAT_MODEL_ID_BYTES = 200;
const MAX_CHAT_MESSAGES = 16;
const MAX_CHAT_MESSAGE_BYTES = 16 * 1024;
const MAX_CHAT_REQUEST_BYTES = 64 * 1024;
const MAX_CHAT_RESPONSE_BYTES = 16 * 1024;
/** A completion is a model writing, not a lookup; the shared clients wait this long for one. */
const CHAT_TIMEOUT_MS = 125 * 1000;
const CHAT_ROLES = ["user", "assistant", "system"];

/** 皮肤广场自己的上限，与 `client-services` 的社区皮肤服务一致。 */
const MAX_COMMUNITY_SEARCH = 128;

// 应用内反馈（`POST /v1/feedback`），长度上限和字段名沿用 Android 的 `FeedbackApi`，服务端与之共用。
const FEEDBACK_PATH = "/v1/feedback";
/** 打字统计「上传到云端」：请求体由本机统计存储（Rust 的 `upload_request`）生成，这里只限大小，服务端另有每份 64 KiB 的上限。 */
const TYPING_STATISTICS_PATH = "/v1/users/me/typing-statistics";
const MAX_TYPING_UPLOAD_BYTES = 64 * 1024;
/** 「绑定 QQ 群助手」：服务端的绑定编号是 1–19 位数字，一个账号最多 10 条，绑定码是 8 位数字。 */
const QQ_LINKS_PATH = "/v1/users/me/qq-links";
const QQ_LINK_ID = /^[1-9][0-9]{0,18}$/;
const QQ_LINK_CODE = /^[0-9]{8}$/;
const MAX_QQ_LINKS = 10;
/** Google 回跳地址的形状：只在 127.0.0.1 的非特权端口上，路径固定（client-services `google_loopback_target`）。端口范围的细节由后端和 `msime_client_google_loopback` 再查一次。 */
const GOOGLE_LOOPBACK_TARGET = /^http:\/\/127\.0\.0\.1:[1-9]\d{3,4}\/callback$/;
const FEEDBACK_TYPES: readonly string[] = ["bug", "suggestion", "dictionary"];
/** 字符数（按码点），与服务端的计数方式一致。 */
const MAX_FEEDBACK_TEXT = 500;
const MAX_FEEDBACK_VERSION_BYTES = 64;
const MAX_FEEDBACK_DIAGNOSTICS = 16;
const MAX_FEEDBACK_DIAGNOSTIC_INPUT_BYTES = 1024;
const MAX_FEEDBACK_DIAGNOSTIC_VALUE_BYTES = 256;
/** 服务端保留的诊断字段；其他字段在发送前丢弃。 */
const FEEDBACK_DIAGNOSTIC_KEYS: readonly string[] = [
  "device",
  "os",
  "app_version",
  "edition",
  "scheme",
  "keyboard_layout",
  "skin",
  "ime_enabled",
  "ime_default",
];
export const MAX_DICTIONARY_EXPORT_BYTES = 384 * 1024 * 1024;
export const MAX_SNAPSHOT_DOWNLOAD_BYTES = 512 * 1024 * 1024;

export function parseResponseContentLength(raw: string): number {
  if (!/^[0-9]+$/.test(raw)) return -1;
  const parsed = Number(raw);
  return Number.isSafeInteger(parsed) ? parsed : -1;
}

/**
 * A publication id, checked before it is put in a path.
 *
 * The service names skins by UUID and the shared service parses one before building the URL. A
 * host that interpolated whatever the page sent would let a page turn a skin id into a different
 * endpoint, so the shape is the check: anything that is not a hyphenated UUID is refused here.
 */
function validUuid(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.toLowerCase() !== "00000000-0000-0000-0000-000000000000" &&
    /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(value)
  );
}

/**
 * Text bound by characters rather than bytes, as the shared service counts it.
 *
 * A description may contain newlines and tabs; a name may not. Everything else that is a control
 * character is refused in both, which is the same rule `valid_text` applies.
 */
function validCommunityText(
  value: unknown,
  minimum: number,
  maximum: number,
  multiline = false,
): value is string {
  if (typeof value !== "string") return false;
  const characters = TextPolicy.characters(value);
  if (characters.length < minimum || characters.length > maximum) return false;
  return (
    TextPolicy.validUnicode(value) &&
    !characters.some((character) => {
      if (multiline && (character === "\n" || character === "\t")) return false;
      return TextPolicy.hasControl(character);
    })
  );
}

function validCommunityKind(value: unknown): value is string {
  return value === "dictionary" || value === "reply";
}

/** `""` is 全部, and the only scope a signed-out browser can ask for. */
function validCommunityScope(value: unknown): value is string {
  return value === "" || value === "mine" || value === "saved";
}

/**
 * 举报可选的理由，按对话框里的顺序，每一项都是发给服务端的确切标签。与 client-services 的 `CommunityReportReason` 是同一份固定列表。
 */
export const COMMUNITY_REPORT_REASONS: readonly string[] = [
  "侵权/抄袭",
  "色情低俗",
  "违法违规",
  "垃圾广告",
  "恶意插件",
  "其他",
];
const MAX_REPORT_DETAIL = 1000;

/**
 * The moderation state the server adds to the user's own items when asked with `fields=moderation`. Optional: other users' items, and servers that predate it, leave it out.
 */
function validModeration(value: unknown): boolean {
  return value === undefined || value === "approved" || value === "pending" || value === "removed";
}

/** A detail path's item id, without the query `fields=moderation` adds. */
function detailId(path: string, prefix: string): string {
  const rest = path.slice(prefix.length);
  const query = rest.indexOf("?");
  return query === -1 ? rest : rest.slice(0, query);
}

/** The service's error code, from `{"error":{"code":...}}`. */
type QqLink = { id: string; created_at: string };

/** 服务端回的一条 QQ 绑定：只取编号和绑定时间，其他字段不往页面传。 */
function qqLink(value: unknown): QqLink | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const link = value as Action;
  if (typeof link.id !== "string" || !QQ_LINK_ID.test(link.id)) return null;
  if (
    typeof link.created_at !== "string" ||
    link.created_at.length > 64 ||
    Number.isNaN(Date.parse(link.created_at))
  ) {
    return null;
  }
  return { id: link.id, created_at: link.created_at };
}

function errorCode(body: string): string | undefined {
  const parsed = parseJson(body);
  const value = parsed === null ? undefined : parsed.error;
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const code = (value as Action).code;
    return typeof code === "string" ? code : undefined;
  }
  return undefined;
}

/**
 * A refusal the user can act on, told apart by the error code in the body rather than by the status alone: content screening (422 `blocked_content`: the text has to change, the service is up), screening that is down for now (503 `screening_unavailable`) and a banned account (403 `account_banned`).
 */
function communityRefusal(status: number, body: string): string | null {
  const code = errorCode(body);
  if (status === 422 && code === "blocked_content") return "community_blocked_content";
  if (status === 503 && code === "screening_unavailable") return "community_screening_unavailable";
  if (status === 403 && code === "account_banned") return "community_account_banned";
  return null;
}

/**
 * What a resource may carry, which depends on what it is.
 *
 * A reply is a prompt and nothing else; a dictionary is between one and 128 entries and no prompt.
 * The shared service refuses the other combinations outright rather than ignoring the extra half,
 * because a "dictionary" carrying a prompt is a resource whose author believed it was something
 * else.
 */
function validResourceContent(kind: unknown, value: unknown): boolean {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const content = value as Action;
  const entries = content.entries;
  const prompt = content.prompt;
  if (kind === "reply") {
    return (
      (entries === undefined || (Array.isArray(entries) && entries.length === 0)) &&
      validCommunityText(prompt, 1, 2000, true)
    );
  }
  if (prompt !== undefined && prompt !== null) return false;
  if (!Array.isArray(entries) || entries.length === 0 || entries.length > 128) return false;
  const seen: string[] = [];
  for (const value of entries as unknown[]) {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
    const entry = value as Action;
    const entryKind = entry.kind;
    const code = entry.code;
    const word = entry.word;
    if (
      typeof entryKind !== "string" ||
      !["pinyin", "wubi", "wubi98", "quick", "english"].includes(entryKind) ||
      !validCommunityText(code, 1, 256) ||
      !validCommunityText(word, 1, 1024) ||
      typeof entry.weight !== "number" ||
      !Number.isSafeInteger(entry.weight) ||
      entry.weight < 0
    ) {
      return false;
    }
    const key = `${entryKind}\u0000${code}\u0000${word}`;
    if (seen.includes(key)) return false;
    seen.push(key);
  }
  return true;
}

function validResourceUuid(value: unknown): value is string {
  return validUuid(value) && value.toLowerCase() !== "00000000-0000-0000-0000-000000000000";
}

function safeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value);
}

function validResponseText(
  value: unknown,
  minimum: number,
  maximum: number,
  multiline = false,
  trim = false,
): value is string {
  if (!validCommunityText(value, 0, maximum, multiline)) return false;
  const text = value as string;
  if (trim && text.trim() !== text) return false;
  return TextPolicy.characters(text.trim()).length >= minimum;
}

function validSkinDesign(value: unknown): boolean {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const design = value as Action;
  const fields = [
    "background",
    "keyBackground",
    "keyForeground",
    "accent",
    "actionBackground",
    "cornerRadius",
    "borderWidth",
    "shadow",
    "pattern",
    "monospaced",
    "keyShape",
    "keyMaterial",
    "keyOpacity",
    "gradientEnd",
    "gradientHorizontal",
    "patternOpacity",
    "customBorderColor",
    "photo",
    "photoShade",
    "photoPosition",
  ];
  if (Object.keys(design).some((field) => !fields.includes(field))) return false;
  const color = (field: string, allowNull = false): boolean => {
    const current = design[field];
    return (
      current === undefined ||
      (allowNull && current === null) ||
      (typeof current === "number" && safeInteger(current) && current >= 0 && current <= 0xffffff)
    );
  };
  const boundedNumber = (
    field: string,
    minimum: number,
    maximum: number,
    allowNull = false,
  ): boolean => {
    const current = design[field];
    return (
      current === undefined ||
      (allowNull && current === null) ||
      (typeof current === "number" &&
        Number.isFinite(current) &&
        current >= minimum &&
        current <= maximum)
    );
  };
  if (
    !["background", "keyBackground", "keyForeground", "accent", "actionBackground"].every((field) =>
      color(field),
    ) ||
    !boundedNumber("cornerRadius", 0, 20) ||
    !boundedNumber("borderWidth", 0, 2) ||
    !boundedNumber("shadow", 0, 0.4) ||
    !boundedNumber("keyOpacity", 0.25, 1, true) ||
    !boundedNumber("patternOpacity", 0, 0.5, true) ||
    !boundedNumber("photoShade", 0, 0.8, true) ||
    !boundedNumber("photoPosition", 0, 1, true)
  ) {
    return false;
  }
  const pattern = design.pattern;
  if (pattern !== undefined && (!safeInteger(pattern) || pattern < 0 || pattern > 3)) return false;
  if (design.monospaced !== undefined && typeof design.monospaced !== "boolean") return false;
  if (design.gradientHorizontal !== undefined && typeof design.gradientHorizontal !== "boolean")
    return false;
  if (
    (design.keyShape !== undefined &&
      design.keyShape !== null &&
      !["rounded", "capsule", "ticket", "pebble"].includes(design.keyShape as string)) ||
    (design.keyMaterial !== undefined &&
      design.keyMaterial !== null &&
      !["flat", "raised", "glass", "paper"].includes(design.keyMaterial as string)) ||
    !color("gradientEnd", true) ||
    !color("customBorderColor", true)
  ) {
    return false;
  }
  const photo = design.photo;
  if (photo !== undefined && photo !== null) {
    if (typeof photo !== "string") return false;
    if (CustomKeyboardSkin.from(design as CustomSkinDocument).photoSource() === null) return false;
  }
  return true;
}

/** 社区皮肤的发布分类 id，与服务端和共享客户端核心一致。 */
const COMMUNITY_SKIN_CATEGORIES: readonly string[] = [
  "nature",
  "guofeng",
  "acg",
  "cute",
  "food",
  "tech",
  "minimal",
  "other",
];

/** 每个返回社区皮肤条目的请求都带上它，服务端才在条目里加上 `category`；不带时服务端不返回该字段。 */
const INCLUDE_CATEGORY = "include=category";

function validCommunitySkinCategory(value: unknown): value is string {
  return typeof value === "string" && COMMUNITY_SKIN_CATEGORIES.includes(value);
}

/** 服务端将来新增的分类读作 `other`，与共享客户端核心的读法一致，页面只会收到它认识的分类。 */
function normalizeCommunitySkinCategory(skin: Action): void {
  if (skin.category !== undefined && !validCommunitySkinCategory(skin.category)) {
    skin.category = "other";
  }
}

function normalizeCommunitySkinCategories(value: unknown): void {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return;
  const object = value as Action;
  if (Array.isArray(object.skins)) {
    for (const item of object.skins as Action[]) normalizeCommunitySkinCategory(item);
  } else {
    normalizeCommunitySkinCategory(object);
  }
}

function validCommunitySkinResponse(value: unknown, expectedId?: string): boolean {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const skin = value as Action;
  return (
    validResourceUuid(skin.id) &&
    (expectedId === undefined || skin.id.toLowerCase() === expectedId.toLowerCase()) &&
    validResponseText(skin.name, 1, 32, false, true) &&
    validResponseText(skin.description, 0, 280, true) &&
    validResponseText(skin.author, 1, 128, false, true) &&
    validSkinDesign(skin.design) &&
    safeInteger(skin.downloads) &&
    skin.downloads >= 0 &&
    safeInteger(skin.rating_count) &&
    skin.rating_count >= 0 &&
    typeof skin.owned === "boolean" &&
    safeInteger(skin.my_rating) &&
    skin.my_rating >= 0 &&
    skin.my_rating <= 5 &&
    typeof skin.rating_average === "number" &&
    Number.isFinite(skin.rating_average) &&
    skin.rating_average >= 0 &&
    skin.rating_average <= 5 &&
    (skin.rating_count !== 0 || skin.rating_average === 0) &&
    validModeration(skin.moderation) &&
    // 早于分类功能的服务端不返回分类；返回时必须是字符串，未知的新分类随后读作 `other`。
    (skin.category === undefined ||
      (typeof skin.category === "string" && skin.category.length <= 32))
  );
}

function validateCommunitySkinResponse(path: string, value: unknown): boolean {
  const listPrefix = "/v1/community/skins?";
  if (path.startsWith(listPrefix)) {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
    const page = value as Action;
    if (
      !Array.isArray(page.skins) ||
      page.skins.length > 20 ||
      typeof page.has_more !== "boolean" ||
      (page.has_more && page.skins.length === 0)
    )
      return false;
    const ids: string[] = [];
    for (const item of page.skins as unknown[]) {
      if (!validCommunitySkinResponse(item)) return false;
      const id = (item as Action).id as string;
      if (ids.includes(id.toLowerCase())) return false;
      ids.push(id.toLowerCase());
    }
    return true;
  }
  const detailPrefix = "/v1/community/skins/";
  if (path.startsWith(detailPrefix)) {
    const id = detailId(path, detailPrefix);
    return validResourceUuid(id) && validCommunitySkinResponse(value, id);
  }
  return true;
}

/** The page and detail contracts are decoded by the UI, so validate them at the native boundary. */
function validCommunityResourceResponse(
  value: unknown,
  expectedKind: unknown,
  expectedId?: string,
): boolean {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const resource = value as Action;
  if (
    !validResourceUuid(resource.id) ||
    (expectedId !== undefined && resource.id.toLowerCase() !== expectedId.toLowerCase()) ||
    !validCommunityKind(resource.kind) ||
    (expectedKind !== undefined && resource.kind !== expectedKind) ||
    !validResponseText(resource.name, 1, 32, false, true) ||
    !validResponseText(resource.description, 0, 280, true) ||
    !validResponseText(resource.author, 1, 128, false, true) ||
    !safeInteger(resource.revision) ||
    resource.revision <= 0 ||
    !safeInteger(resource.saves) ||
    resource.saves < 0 ||
    !safeInteger(resource.rating_count) ||
    resource.rating_count < 0 ||
    typeof resource.saved !== "boolean" ||
    typeof resource.owned !== "boolean" ||
    !safeInteger(resource.my_rating) ||
    resource.my_rating < 0 ||
    resource.my_rating > 5 ||
    typeof resource.rating_average !== "number" ||
    !Number.isFinite(resource.rating_average) ||
    resource.rating_average < 0 ||
    resource.rating_average > 5 ||
    (resource.rating_count === 0 && resource.rating_average !== 0) ||
    !validResourceContent(resource.kind, resource.content) ||
    !validModeration(resource.moderation)
  ) {
    return false;
  }
  return true;
}

function validateCommunityResourceResponse(path: string, value: unknown): boolean {
  const listMatch = /^\/v1\/community\/resources\?kind=(dictionary|reply)(?:&|$)/.exec(path);
  if (listMatch !== null) {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
    const page = value as Action;
    if (
      !Array.isArray(page.items) ||
      page.items.length > 20 ||
      typeof page.has_more !== "boolean" ||
      (page.has_more && page.items.length === 0)
    ) {
      return false;
    }
    const ids: string[] = [];
    for (const item of page.items as unknown[]) {
      if (!validCommunityResourceResponse(item, listMatch[1])) return false;
      const id = (item as Action).id as string;
      if (ids.includes(id.toLowerCase())) return false;
      ids.push(id.toLowerCase());
    }
    return true;
  }
  const detailPrefix = "/v1/community/resources/";
  if (path.startsWith(detailPrefix)) {
    const id = detailId(path, detailPrefix);
    return validResourceUuid(id) && validCommunityResourceResponse(value, undefined, id);
  }
  return true;
}

function validCommunityResourcePublication(value: Action, expectedId: unknown): boolean {
  return (
    validResourceUuid(value.id) &&
    typeof expectedId === "string" &&
    value.id.toLowerCase() === expectedId.toLowerCase() &&
    safeInteger(value.revision) &&
    value.revision > 0
  );
}

function validCommunityResourceApplication(
  value: Action,
  expectedResourceRevision: unknown,
  dictionaryRevision: unknown,
): boolean {
  return (
    safeInteger(expectedResourceRevision) &&
    expectedResourceRevision > 0 &&
    safeInteger(dictionaryRevision) &&
    dictionaryRevision >= 0 &&
    safeInteger(value.revision) &&
    safeInteger(value.imported) &&
    value.imported >= 0 &&
    value.imported <= 128 &&
    safeInteger(value.resource_revision) &&
    value.resource_revision === expectedResourceRevision &&
    value.revision >= dictionaryRevision &&
    value.revision - dictionaryRevision === value.imported
  );
}

function validCommunitySaved(value: Action, expected: unknown): boolean {
  return typeof expected === "boolean" && value.saved === expected;
}

function validCommunityRating(value: Action, expected: unknown): boolean {
  return safeInteger(expected) && expected >= 1 && expected <= 5 && value.stars === expected;
}

function validCommunityDeletion(value: Action): boolean {
  return value.deleted === true;
}

/** The community pages have their own wording; an `account_*` code arrives as the generic one. */
function communityStatus(status: number): string {
  if (status === 400) return "community_invalid";
  if (status === 401) return "community_unauthorized";
  if (status === 403) return "community_forbidden";
  if (status === 404) return "community_not_found";
  if (status === 409) return "community_conflict";
  if (status === 429) return "community_rate_limited";
  return "community_unavailable";
}

function communityResponseLimit(path: string): number {
  if (path.startsWith("/v1/community/resources?")) return MAX_COMMUNITY_RESOURCE_PAGE_BYTES;
  if (path.startsWith("/v1/community/resources/")) return MAX_COMMUNITY_RESOURCE_DETAIL_BYTES;
  return MAX_JSON_RESPONSE_BYTES;
}

function error(code: string): string {
  return JSON.stringify({ ok: false, error: code });
}

function success(value: unknown): string {
  return JSON.stringify({ ok: true, value });
}

/** 账号桥接信封只有布尔值 `true` 才表示成功，拒绝其它 truthy 值。 */
export function strictAccountOk(value: unknown): value is true {
  return value === true;
}

/** 原生账号成功信封必须同时带有非空 value，避免缺失值流入后续请求。 */
export function accountReplyValue<T>(reply: unknown): T | null {
  if (reply === null || typeof reply !== "object" || Array.isArray(reply)) return null;
  const value: unknown = (reply as { value?: unknown }).value;
  return strictAccountOk((reply as { ok?: unknown }).ok) && value !== undefined && value !== null
    ? (value as T)
    : null;
}

function validString(value: unknown, maximum: number, allowEmpty = false): value is string {
  return (
    typeof value === "string" &&
    (allowEmpty || value.length > 0) &&
    utf8Length(value) <= maximum &&
    !TextPolicy.hasControl(value) &&
    TextPolicy.validUnicode(value)
  );
}

function validToken(value: unknown): value is string {
  return typeof value === "string" && value.length === 64 && /^[0-9a-f]+$/.test(value);
}

/** Account resources use digest ids; reject path-like input before attaching a session token. */
function validResourceId(value: unknown): value is string {
  return typeof value === "string" && value.length === 64 && /^[0-9a-f]+$/.test(value);
}

function boundedUtf8(value: unknown, maximumBytes: number): value is string {
  return typeof value === "string" && utf8Length(value) <= maximumBytes;
}

/** Chat paragraphs allow tabs and line breaks, but not other Unicode controls. */
function validChatText(value: unknown, maximumBytes: number): value is string {
  if (
    typeof value !== "string" ||
    value.trim().length === 0 ||
    !boundedUtf8(value, maximumBytes) ||
    !TextPolicy.validUnicode(value)
  )
    return false;
  return !TextPolicy.characters(value).some(
    (character) =>
      character !== "\t" &&
      character !== "\n" &&
      character !== "\r" &&
      TextPolicy.hasControl(character),
  );
}

/** 反馈正文：去掉首尾空白后不能为空，最多 500 个字符，除换行和制表符外不含控制字符。 */
function validFeedbackText(value: unknown): value is string {
  return (
    typeof value === "string" &&
    TextPolicy.characters(value).length <= MAX_FEEDBACK_TEXT &&
    TextPolicy.validMultiline(value, MAX_FEEDBACK_TEXT * 4, true)
  );
}

/** 把 `value` 截到 UTF-8 编码最多 `maximumBytes` 字节，不拆开字符。 */
function clipUtf8(value: string, maximumBytes: number): string {
  let clipped = "";
  let bytes = 0;
  for (const character of TextPolicy.characters(value)) {
    const size = utf8Length(character);
    if (bytes + size > maximumBytes) break;
    clipped += character;
    bytes += size;
  }
  return clipped;
}

/**
 * 反馈可附带的诊断信息：`null`（用户没打开「附带诊断信息」），或最多 16 个字符串字段的对象。结果只保留服务端接受的字段，控制字符换成空格，去掉首尾空白，截到 256 字节，并丢弃空值；值不属于这两种形状时返回 `undefined`。
 */
function feedbackDiagnostics(value: unknown): Record<string, string> | null | undefined {
  if (value === null || value === undefined) return null;
  if (typeof value !== "object" || Array.isArray(value)) return undefined;
  const raw = value as Record<string, unknown>;
  const keys: string[] = Object.keys(raw);
  if (keys.length > MAX_FEEDBACK_DIAGNOSTICS) return undefined;
  if (
    keys.some(
      (key: string): boolean =>
        !boundedUtf8(raw[key], MAX_FEEDBACK_DIAGNOSTIC_INPUT_BYTES) ||
        !TextPolicy.validUnicode(raw[key] as string),
    )
  )
    return undefined;
  const clean: Record<string, string> = {};
  for (const key of FEEDBACK_DIAGNOSTIC_KEYS) {
    if (!keys.includes(key)) continue;
    const spaced: string = TextPolicy.characters(raw[key] as string)
      .map((character: string): string => (TextPolicy.hasControl(character) ? " " : character))
      .join("");
    const trimmed: string = clipUtf8(spaced.trim(), MAX_FEEDBACK_DIAGNOSTIC_VALUE_BYTES).trim();
    if (trimmed.length > 0) clean[key] = trimmed;
  }
  return clean;
}

function validCandidateKind(value: unknown): value is string {
  return (
    typeof value === "string" &&
    ["pinyin", "jianpin", "wubi", "wubi98", "quick", "english"].includes(value)
  );
}

function validCandidateScheme(value: unknown): value is string {
  return value === "pinyin" || value === "shuangpin";
}

function validCandidateProfile(value: unknown): value is string {
  return value === "xiaohe" || value === "ziranma" || value === "microsoft" || value === "shoudao";
}

function validCandidateRankingMode(value: unknown): value is string {
  return (
    value === "disabled" ||
    value === "pin" ||
    value === "halve" ||
    value === "linear" ||
    value === "promote"
  );
}

/** The account API uses dictionary-specific syntax for codes, not arbitrary text. */
function validDictionaryValue(
  kind: string,
  code: unknown,
  word: unknown,
  weight: unknown,
  newValue: boolean,
): boolean {
  if (
    !validString(code, 256) ||
    !validString(word, 1024) ||
    !Number.isInteger(weight) ||
    (weight as number) < 0 ||
    (weight as number) > 2147483647
  ) {
    return false;
  }
  const codeText = code as string;
  const wordText = word as string;
  if (kind === "pinyin") return /^[a-z' ]+$/.test(codeText);
  if (kind === "wubi" || kind === "wubi98") {
    return codeText.length <= 4 && /^[a-z]+$/.test(codeText);
  }
  if (kind === "quick") {
    return (
      codeText.length <= 32 &&
      (newValue ? /^[a-z]+$/.test(codeText) : /^[a-z0-9]+$/.test(codeText)) &&
      wordText.length <= MAX_QUICK_PHRASE_UTF16
    );
  }
  if (kind === "english") {
    return codeText.length <= 64 && /^[A-Za-z'-]+$/.test(codeText);
  }
  return false;
}

function validDictionaryIdentity(kind: string, code: unknown, word: unknown): boolean {
  if (!validString(code, 256) || !validString(word, 1024)) return false;
  const codeText = code as string;
  if (kind === "pinyin") return /^[a-z' ]+$/.test(codeText);
  if (kind === "wubi" || kind === "wubi98") {
    return /^[a-z]+$/.test(codeText);
  }
  if (kind === "quick") return /^[a-z0-9]+$/.test(codeText);
  if (kind === "english") return /^[A-Za-z'-]+$/.test(codeText);
  return false;
}

function validCandidateValue(kind: string, code: unknown, word: unknown): boolean {
  return (
    validString(code, 256) &&
    validString(word, 1024) &&
    (kind !== "quick" || (word as string).length <= MAX_QUICK_PHRASE_UTF16)
  );
}

function parseBody(body: string): Action | null {
  if (body.length === 0 || utf8Length(body) > MAX_ACTION_BYTES) return null;
  try {
    const value: unknown = JSON.parse(body);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Action)
      : null;
  } catch {
    return null;
  }
}

function mapStatus(status: number): string {
  if (status === 400) return "account_invalid";
  if (status === 401 || status === 403) return "account_unauthorized";
  if (status === 404) return "account_unavailable";
  if (status === 409) return "account_conflict";
  if (status === 429) return "account_rate_limited";
  return "account_unavailable";
}

function parseJson(body: string, maximumBytes: number = MAX_JSON_RESPONSE_BYTES): Action | null {
  if (utf8Length(body) > maximumBytes) return null;
  try {
    const value: unknown = JSON.parse(body);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Action)
      : null;
  } catch {
    return null;
  }
}

/** 驱动要宿主发出的一个请求；驱动给的 `body` 是 JSON 文本，这里已解析成对象交给传输层。 */
type ClipboardDriverRequest = {
  method: string;
  path: string;
  body: Record<string, unknown> | undefined;
};

/** 驱动的一步：要发请求（带不透明的 `state`）、完成或失败，三种都带最新的密钥串。 */
type ClipboardDriverStep = {
  keystore: string;
  request?: ClipboardDriverRequest;
  state?: unknown;
  done?: unknown;
  error?: string;
};

const CLIPBOARD_DRIVER_METHODS: readonly string[] = ["GET", "POST", "PUT", "DELETE"];

/** 这些 403 说的是账号或功能的状态，不是令牌失效：原样交给调用方，不刷新令牌重试。 */
const FORBIDDEN_NOT_CREDENTIAL: readonly string[] = ["account_banned", "clipboard_sync_disabled"];

/**
 * 读原生驱动的应答 `{ok:true,value}`。形状不对（包括原生拒绝了输入的 `{ok:false}`）时返回 null，调用方按服务不可用处理。
 *
 * 失败一步的 `code` 原样保留：设置页和键盘按 `clipboard_device_pending`、`clipboard_sync_disabled` 这些码说话。
 */
export function parseClipboardDriverStep(reply: string): ClipboardDriverStep | null {
  let envelope: unknown;
  try {
    envelope = JSON.parse(reply);
  } catch {
    return null;
  }
  const value: Action | null = accountReplyValue<Action>(envelope);
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const keystore: unknown = value.keystore;
  if (typeof keystore !== "string" || keystore.length === 0) return null;
  const request: unknown = value.request;
  if (request !== undefined) {
    if (request === null || typeof request !== "object" || Array.isArray(request)) return null;
    const raw = request as Action;
    if (
      typeof raw.method !== "string" ||
      !CLIPBOARD_DRIVER_METHODS.includes(raw.method) ||
      typeof raw.path !== "string" ||
      !raw.path.startsWith("/v1/users/me/clipboard") ||
      value.state === undefined
    )
      return null;
    let body: Record<string, unknown> | undefined = undefined;
    if (typeof raw.body === "string") {
      const parsed: Action | null = parseBody(raw.body);
      if (parsed === null) return null;
      body = parsed;
    } else if (raw.body !== null && raw.body !== undefined) {
      return null;
    }
    return { keystore, request: { method: raw.method, path: raw.path, body }, state: value.state };
  }
  if (value.error !== undefined) {
    const failure: unknown = value.error;
    if (failure === null || typeof failure !== "object" || Array.isArray(failure)) return null;
    const code: unknown = (failure as Action).code;
    // 与驱动认可的错误码同一个字符集（client-core 的 `is_bounded_ascii_identifier`），驱动给的码一个都不丢。
    if (typeof code !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(code)) return null;
    return { keystore, error: code };
  }
  if (value.done === undefined || value.done === null) return null;
  return { keystore, done: value.done };
}

/**
 * Validate the one-record change page used to guard a native snapshot apply.
 *
 * Harmony applies complete Engine snapshots rather than replaying cloud mutations, so it only
 * needs to know whether anything changed. It still has to reject the same impossible cursors the
 * fixed Apple client rejects; treating an inconsistent empty page as current could overwrite a
 * newer dictionary with the preview the user saw earlier.
 */
export function dictionaryChangePageChanged(body: string, after: number): boolean | null {
  if (!Number.isSafeInteger(after) || after < 0 || utf8Length(body) > 256 * 1024) return null;
  const page = parseJson(body);
  if (page === null || !Array.isArray(page.changes) || page.changes.length > 1) return null;
  if (!Number.isSafeInteger(page.next) || (page.next as number) < 0) return null;
  if (typeof page.has_more !== "boolean") return null;
  const next = page.next as number;
  if (page.changes.length === 0) return next === after && page.has_more === false ? false : null;
  const change = page.changes[0];
  if (change === null || typeof change !== "object" || Array.isArray(change)) return null;
  const revision = (change as Action).revision;
  if (!Number.isSafeInteger(revision) || (revision as number) <= after || revision !== next)
    return null;
  return true;
}

function validateUser(value: unknown): value is Session["user"] {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const user = value as Action;
  return (
    validString(user.id, 256) &&
    validString(user.display_name, 256, true) &&
    TextPolicy.characters(user.display_name).length <= 64 &&
    validString(user.created_at, 128, true)
  );
}

function validateSession(value: unknown): value is Session {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const session = value as Action;
  return (
    validToken(session.access_token) &&
    validToken(session.refresh_token) &&
    session.token_type === "Bearer" &&
    safeInteger(session.expires_at) &&
    session.expires_at <= Date.now() + MAX_SESSION_MILLISECONDS &&
    validateUser(session.user)
  );
}

/** 只读出刚从文件载入的会话属于哪个账号，不跨进程缓存；会话缺失、格式不对或校验不过时返回 null。 */
export function storedSessionUserId(saved: string | null): string | null {
  if (saved === null || utf8Length(saved) > MAX_SESSION_BYTES) return null;
  try {
    const value: unknown = JSON.parse(saved);
    return validateSession(value) ? value.user.id : null;
  } catch {
    return null;
  }
}

function sessionFromTokens(value: Action): Session | null {
  if (
    !validToken(value.access_token) ||
    !validToken(value.refresh_token) ||
    value.token_type !== "Bearer" ||
    !safeInteger(value.expires_in) ||
    value.expires_in <= 0 ||
    value.expires_in > MAX_SESSION_SECONDS ||
    !validateUser(value.user)
  ) {
    return null;
  }
  return {
    access_token: value.access_token,
    refresh_token: value.refresh_token,
    token_type: "Bearer",
    expires_at: Date.now() + Math.max(1, value.expires_in) * 1000,
    user: value.user,
  };
}

export class AccountCloudBridge {
  private readonly transport: AccountTransport;
  private readonly store: AccountSessionStore;
  private session: Session | null = null;
  private generation = 0;
  private refreshing: Promise<CredentialReply> | null = null;
  private adoptedStoredSession = false;
  private readonly anonymousStore: AccountSessionStore | undefined;
  private anonymous: AccountCloudBridge | null = null;
  private readonly clipboardE2eHost: CloudClipboardE2eHost | undefined;

  /**
   * `anonymousStore` 是设备的匿名账号，供无人登录时可以用它发出的请求使用（应用内反馈，对应 Android 的 `ACCOUNT_OR_ANONYMOUS`）。没有匿名账号的宿主不传它，这些请求就需要已登录的账号。
   *
   * `clipboardE2e` 是云剪贴板端到端加密的原生驱动和本机密钥串。不传它的桥接不提供云剪贴板的列表、上传和设备管理（服务端只接受密文），它清除会话时也就没有密钥串要一起删。
   */
  constructor(
    transport: AccountTransport,
    store: AccountSessionStore,
    anonymousStore?: AccountSessionStore,
    clipboardE2e?: CloudClipboardE2eHost,
  ) {
    this.transport = transport;
    this.store = store;
    this.anonymousStore = anonymousStore;
    this.clipboardE2eHost = clipboardE2e;
    const saved = store.load();
    if (saved !== null && utf8Length(saved) <= MAX_SESSION_BYTES) {
      try {
        const value: unknown = JSON.parse(saved);
        if (validateSession(value)) this.session = value;
        else this.discardStoredSession();
      } catch {
        this.discardStoredSession();
      }
    } else if (saved !== null) {
      this.discardStoredSession();
    }
  }

  async handle(document: string): Promise<string> {
    const action = parseBody(document);
    if (action === null || typeof action.operation !== "string") return error("account_invalid");
    const operation = action.operation;
    try {
      switch (operation) {
        case "status":
          return success({ user: this.session?.user ?? null });
        case "providers":
          return this.requestPublic("GET", "/v1/auth/providers");
        case "request_code":
          return await this.requestCode(action);
        case "login":
          return await this.login(action);
        case "google_login":
          return await this.googleLogin(action);
        case "profile":
          return await this.profile();
        case "rename":
          return await this.rename(action);
        case "logout":
          return await this.logout(action);
        case "delete_account":
          return await this.deleteAccount();
        case "clear_expired":
          await this.signOut();
          return success({});
        case "clipboard":
          return await this.clipboard(action);
        case "dictionary":
          return await this.dictionary(action);
        case "chat":
          return await this.chat(action);
        case "community_skin":
          return await this.community(action);
        case "community_resource":
          return await this.communityResource(action);
        case "feedback":
          return await this.feedback(action);
        case "qq_links":
          return await this.qqLinks();
        case "qq_link_redeem":
          return await this.qqLinkRedeem(action);
        case "qq_link_delete":
          return await this.qqLinkDelete(action);
        default:
          return error("account_invalid");
      }
    } catch (cause) {
      // The community pages decode a different vocabulary from the account ones, so a thrown
      // transport failure has to arrive in the one its caller has wording for.
      if (operation === "community_skin" || operation === "community_resource") {
        return error("community_unavailable");
      }
      // 请求在传输层就失败了（断网、连接被重置）：鸿蒙抛出的是 BusinessError 而不是 Error，原来落到 account_unavailable，页面写「账号服务暂不可用」「聊天服务暂不可用」，让人以为是服务坏了。改报 account_network，账号页写「网络连接失败」，对话页按默认文案写「连接失败，请检查网络后重试」。对话请求连 Error 也这样报：它的 Error 都来自传输层。
      if (operation === "chat" || !(cause instanceof Error)) return error("account_network");
      return error(cause.message);
    }
  }

  /**
   * Who is signed in, for a caller that has to check before writing to the device.
   *
   * Applying cloud settings names the account they were read from. If the session changed in
   * between — a sign-out and a different sign-in while the confirmation was on screen — the values
   * on screen belong to someone else, and writing them is the one failure this sync could have that
   * the user would not be able to undo from here.
   */
  currentUserId(): string | null {
    const session = this.session;
    return session?.user.id ?? null;
  }

  /** Whether this bridge's login is still the one in the shared store. A token rotation by another process conservatively invalidates a keyboard listing. */
  matchesStoredSession(): boolean {
    const current: Session | null = this.session;
    const stored: Session | null = this.storedSession();
    return (
      current !== null &&
      stored !== null &&
      current.user.id === stored.user.id &&
      current.refresh_token === stored.refresh_token
    );
  }

  /**
   * A keyboard send may refresh its own token, but must not follow a different process's login or rotation.
   *
   * 上传经端到端加密驱动：文字在本机加密后才发出。本机还在等待批准时驱动以 `clipboard_device_pending` 拒绝，不上传。
   */
  async addClipboardForCurrentSession(text: string): Promise<string> {
    if (!CloudClipboardPolicy.validText(text) || !this.matchesStoredSession())
      return error("account_cancelled");
    const guard = (): boolean => !this.adoptedStoredSession && this.matchesStoredSession();
    try {
      return await this.clipboardE2e({ op: "add", text }, guard);
    } catch (cause) {
      return error(cause instanceof Error ? cause.message : "account_unavailable");
    }
  }

  /** A marker for native operations that may write after several asynchronous account calls. */
  sessionGeneration(): number {
    return this.generation;
  }

  /**
   * An authenticated request for the AI skin run, whose bodies this bridge does not compose.
   *
   * The run's four requests carry documents the shared client decides — the system prompt, the
   * artwork prompt the model wrote — so validating them a second time here would be this host
   * having an opinion about a contract it does not own. What it does own is the session, the
   * expiry and the generation, which is why the request still goes through the bridge.
   */
  async aiSkinRequest(
    method: string,
    path: string,
    body?: Record<string, unknown>,
    timeoutMs?: number,
    requestTag?: string,
  ): Promise<{ value?: Action; error?: string }> {
    if (!path.startsWith("/v1/")) return { error: "ai_skin_invalid" };
    const result = await this.authenticatedJson(method, path, body, timeoutMs, requestTag);
    if (result.error === undefined) return result;
    // The AI skin page decodes its own vocabulary, and an account code would arrive as a sentence
    // about signing in rather than about the picture that failed.
    return {
      error:
        result.error === "account_unauthorized"
          ? "ai_skin_unauthorized"
          : result.error === "account_invalid"
            ? "ai_skin_invalid"
            : result.error === "account_rate_limited"
              ? "ai_skin_busy"
              : "ai_skin_unavailable",
    };
  }

  /** Abort the HTTP request currently owned by one AI skin generation. */
  cancelAiSkinRequests(requestTag: string): void {
    if (requestTag.length > 0) this.transport.cancelRequests?.(requestTag);
  }

  /** The account preference schema and document, for the native half of the settings sync. */
  async preferenceSchema(): Promise<{ value?: Action; error?: string }> {
    return await this.authenticatedJson("GET", "/v1/users/me/preferences/schema");
  }

  async loadPreferenceDocument(): Promise<{ value?: Action; error?: string }> {
    return await this.authenticatedJson("GET", "/v1/users/me/preferences");
  }

  async putPreferenceDocument(
    value: Record<string, unknown>,
  ): Promise<{ value?: Action; error?: string }> {
    return await this.authenticatedJson("PUT", "/v1/users/me/preferences", value);
  }

  /** Native-only raw download used for bounded snapshot files; never exposed to the WebView. */
  async rawAuthenticated(method: string, path: string): Promise<AccountTransportResponse> {
    const result = await this.authorizedResponse(method, path);
    if (result.response !== undefined) return result.response;
    const status: number =
      result.error === "account_cancelled"
        ? 499
        : result.error === "account_unauthorized"
          ? 401
          : 503;
    return { status, body: "" };
  }

  /** Stream a large authenticated response into a host-private file with one token retry. */
  async downloadAuthenticated(
    path: string,
    destination: string,
    maximumBytes: number,
    mediaType: string,
  ): Promise<{ bytes?: number; error?: string }> {
    if (
      !path.startsWith("/v1/") ||
      path.includes("\\") ||
      destination.length === 0 ||
      !Number.isSafeInteger(maximumBytes) ||
      maximumBytes <= 0 ||
      maximumBytes > MAX_SNAPSHOT_DOWNLOAD_BYTES ||
      !["text/plain", "application/x-ndjson"].includes(mediaType) ||
      this.transport.download === undefined
    )
      return { error: "account_invalid" };
    const result = await this.authorizedDownload(path, destination, maximumBytes, mediaType);
    if (result.response === undefined) return { error: result.error ?? "account_unavailable" };
    const response = result.response;
    if (response.status < 200 || response.status >= 300)
      return { error: mapStatus(response.status) };
    if (
      response.bytes <= 0 ||
      response.bytes > maximumBytes ||
      response.contentType !== mediaType ||
      (response.contentLength !== undefined && response.contentLength !== response.bytes)
    )
      return { error: "account_unavailable" };
    return { bytes: response.bytes };
  }

  /** Read the dictionary revision used by snapshot restore's optimistic concurrency check. */
  async currentDictionaryRevision(): Promise<{ revision?: number; error?: string }> {
    const result = await this.authenticatedJson(
      "GET",
      "/v1/users/me/dictionaries/quick/catalog?q=&offset=0&limit=100&scheme=pinyin&profile=xiaohe",
    );
    if (result.error !== undefined || result.value === undefined || result.value === null)
      return { error: result.error ?? "account_unavailable" };
    const revision = result.value.revision;
    if (typeof revision !== "number" || !Number.isSafeInteger(revision) || revision < 0)
      return { error: "account_unavailable" };
    return { revision };
  }

  /** Stream one already-inspected private snapshot with token refresh and generation checks. */
  async restoreSnapshotAuthenticated(
    source: string,
    revision: number,
    expectedSha256: string,
  ): Promise<{ value?: Action; error?: string }> {
    if (
      !source.startsWith("/") ||
      source.length > 16384 ||
      source.includes("\u0000") ||
      !Number.isSafeInteger(revision) ||
      revision < 0 ||
      !validResourceId(expectedSha256) ||
      this.transport.uploadSnapshot === undefined
    )
      return { error: "account_invalid" };
    const result = await this.authorizedSnapshotUpload(source, revision, expectedSha256);
    if (result.response === undefined) return { error: result.error ?? "account_unavailable" };
    const response = result.response;
    if (response.status < 200 || response.status >= 300)
      return { error: mapStatus(response.status) };
    const value = parseJson(response.body);
    if (
      value === null ||
      value.reset !== true ||
      typeof value.revision !== "number" ||
      !Number.isSafeInteger(value.revision) ||
      value.revision <= revision
    )
      return { error: "account_unavailable" };
    return { value };
  }

  private async requestCode(action: Action): Promise<string> {
    const provider = action.provider;
    const target = action.target;
    if (
      !validString(provider, 16) ||
      !["email", "phone", "google"].includes(provider) ||
      !validString(target, 320) ||
      target.trim() !== target ||
      // Google 的 target 是设置应用在本机监听的回跳地址（`msime_client_google_loopback` 的 target），不是用户填的。
      (provider === "google" && !GOOGLE_LOOPBACK_TARGET.test(target))
    )
      return error("account_invalid");
    return this.requestPublic("POST", "/v1/auth/challenges", {
      provider,
      target,
      purpose: "login",
    });
  }

  private async login(action: Action): Promise<string> {
    if (
      !validString(action.challenge_id, 256) ||
      !validString(action.credential, 6) ||
      !/^\d{6}$/.test(action.credential)
    )
      return error("account_invalid");
    return this.signIn(action.challenge_id, action.credential);
  }

  /** 用 Google 回跳带回的授权码登录；后端用 challenge 里保管的 PKCE 和 client secret 去换 token。授权码的规则与 client-services 的 `validate_google_login` 相同：可见 ASCII，最长 2048 字节。 */
  private async googleLogin(action: Action): Promise<string> {
    if (
      !validString(action.challenge_id, 256) ||
      !validString(action.code, 2048) ||
      !/^[\x21-\x7e]+$/.test(action.code)
    )
      return error("account_invalid");
    return this.signIn(action.challenge_id, action.code);
  }

  private async signIn(challengeId: string, credential: string): Promise<string> {
    this.generation++;
    this.refreshing = null;
    const generation = this.generation;
    const response = await this.transport.request("POST", "/v1/auth/login", undefined, {
      challenge_id: challengeId,
      credential,
    });
    if (generation !== this.generation) return error("account_cancelled");
    if (response.status < 200 || response.status >= 300) return error(mapStatus(response.status));
    const value = parseJson(response.body);
    if (value === null) return error("account_unavailable");
    const session: Session | null = sessionFromTokens(value);
    if (session === null) return error("account_unavailable");
    try {
      // Under the lock, so a refresh the keyboard is in the middle of cannot write the previous session over this one.
      return await this.locked(async (): Promise<string> => {
        if (generation !== this.generation) return error("account_cancelled");
        // 换了一个账号登录时，上一个账号的云剪贴板密钥串不留给它：设备身份按账号重新生成，在新账号下重新登记。
        if (this.storedSession()?.user.id !== session.user.id)
          this.clipboardE2eHost?.clearKeystore();
        this.store.save(JSON.stringify(session));
        this.session = session;
        return success({ user: session.user });
      });
    } catch {
      return error("account_unavailable");
    }
  }

  private async rename(action: Action): Promise<string> {
    if (
      !validString(action.display_name, 256) ||
      TextPolicy.characters(action.display_name).length > 64 ||
      action.display_name.trim() !== action.display_name
    ) {
      return error("account_invalid");
    }
    const generation = this.generation;
    const userId = this.session?.user.id ?? null;
    const renamed = await this.authorizedResponse("PATCH", "/v1/users/me", {
      display_name: action.display_name,
    });
    if (renamed.response === undefined) return error(renamed.error ?? "account_unavailable");
    if (renamed.response.status < 200 || renamed.response.status >= 300) {
      return error(mapStatus(renamed.response.status));
    }
    // PATCH is a no-content operation in the shared service. Read the canonical profile back so
    // the page and the persisted native session agree even if the service normalizes the name.
    return await this.profile(generation, userId);
  }

  /** Read the canonical profile and bind its user to the session that authorized this reply. */
  private async profile(
    expectedGeneration: number = this.generation,
    expectedUserId: string | null = this.session?.user.id ?? null,
  ): Promise<string> {
    if (expectedGeneration !== this.generation) {
      return error("account_cancelled");
    }
    const result = await this.authorizedResponse("GET", "/v1/users/me");
    if (result.response === undefined || result.token === undefined) {
      return error(result.error ?? "account_unavailable");
    }
    if (result.response.status < 200 || result.response.status >= 300) {
      return error(mapStatus(result.response.status));
    }
    const value = parseJson(result.response.body);
    if (value === null || !validateUser(value.user) || !Array.isArray(value.identities)) {
      return error("account_unavailable");
    }
    const current = this.session;
    if (
      current === null ||
      expectedGeneration !== this.generation ||
      current.access_token !== result.token ||
      current.user.id !== expectedUserId ||
      value.user.id !== expectedUserId
    ) {
      return error("account_cancelled");
    }
    const user: Session["user"] = value.user;
    try {
      // Only the user record is written back, onto whatever session the store holds now: the other process may have rotated the tokens, or the user signed in again, since this request was sent, and writing the tokens held here would put a spent or superseded refresh token back on disk.
      const saved: boolean = await this.locked(async (): Promise<boolean> => {
        if (expectedGeneration !== this.generation || this.session === null) return false;
        const stored: Session | null = this.storedSession();
        if (stored === null || stored.user.id !== expectedUserId) return false;
        this.store.save(JSON.stringify({ ...stored, user }));
        this.session = { ...this.session, user };
        return true;
      });
      return saved ? success(value) : error("account_cancelled");
    } catch {
      return error("account_unavailable");
    }
  }

  private async logout(action: Action): Promise<string> {
    if (typeof action.all !== "boolean") return error("account_invalid");
    const result = await this.authenticated("POST", "/v1/auth/logout", { all: action.all });
    const reply = parseJson(result);
    if (reply !== null && strictAccountOk(reply.ok)) await this.signOut();
    return result;
  }

  private async deleteAccount(): Promise<string> {
    const result = await this.authenticated("DELETE", "/v1/users/me");
    const reply = parseJson(result);
    if (reply !== null && strictAccountOk(reply.ok)) await this.signOut();
    return result;
  }

  /**
   * 云剪贴板的动作。列表、上传和设备管理经端到端加密驱动（服务端只存密文，搜索也在解密后由驱动在本机做）；删除和开关直接调用原来的接口。
   *
   * `devices` 是驱动的 `status`；`approve`、`reject`、`remove` 带 `device_id`，`reset` 不带参数，这五个动作的结果都是驱动 `status` 的形状。
   */
  private async clipboard(action: Action): Promise<string> {
    const operation = action.clipboard_operation;
    if (operation === "list") {
      if (!validString(action.search, MAX_SEARCH, true)) return error("account_invalid");
      return this.clipboardE2e({ op: "list", search: action.search });
    }
    if (operation === "add") {
      // Clipboard text keeps its line breaks and tabs, as the shared client allows; the generic string check here would refuse every multi-line copy.
      if (!CloudClipboardPolicy.validText(action.text)) return error("account_invalid");
      return this.clipboardE2e({ op: "add", text: action.text });
    }
    if (operation === "devices") return this.clipboardE2e({ op: "status" });
    if (operation === "approve" || operation === "reject" || operation === "remove") {
      // 设备编号会进请求路径，先按协议的形状（32 位小写十六进制）挡住别的内容。
      if (typeof action.device_id !== "string" || !/^[0-9a-f]{32}$/.test(action.device_id))
        return error("account_invalid");
      return this.clipboardE2e({ op: operation, device_id: action.device_id });
    }
    if (operation === "reset") return this.clipboardE2e({ op: "reset" });
    if (operation === "delete") {
      if (!validResourceId(action.id)) return error("account_invalid");
      return this.authenticated("DELETE", `/v1/users/me/clipboard/${action.id}`);
    }
    if (operation === "set_enabled") {
      if (typeof action.enabled !== "boolean") return error("account_invalid");
      const result = await this.authenticated("PUT", "/v1/users/me/clipboard/settings", {
        enabled: action.enabled,
      });
      const reply = parseJson(result);
      return reply !== null && strictAccountOk(reply.ok)
        ? success({ enabled: action.enabled })
        : result;
    }
    return error("account_invalid");
  }

  /**
   * 跑一次端到端加密驱动：开始操作，按驱动的要求带着账号凭据发请求，把状态码和正文原样交回——非 2xx 也交回，由驱动按错误码判断——直到它给出结果或错误。
   *
   * 每一步交回的密钥串与已保存的不同，就先保存再发下一个请求：驱动刚生成的设备身份要是没存下就登记到了服务端，下次会再登记一台。会话在途中被登出（本进程或另一个进程）时立即停下且不再保存，免得把刚随会话删掉的密钥串写回去。传输失败和令牌被拒按原来的方式报错，不交回驱动。
   */
  private async clipboardE2e(start: Action, sessionGuard?: () => boolean): Promise<string> {
    const host: CloudClipboardE2eHost | undefined = this.clipboardE2eHost;
    if (host === undefined) return error("account_unavailable");
    const session: Session | null = this.session;
    if (session === null) return error("account_unauthorized");
    const generation: number = this.generation;
    const signedIn = (): boolean =>
      generation === this.generation &&
      this.session !== null &&
      this.storedSession()?.user.id === session.user.id &&
      (sessionGuard === undefined || sessionGuard());
    let saved: string | null = host.loadKeystore();
    let step: ClipboardDriverStep | null = parseClipboardDriverStep(
      host.drive(JSON.stringify({ start, keystore: saved, device_name: host.deviceName() })),
    );
    for (let round = 0; step !== null && round < MAX_CLIPBOARD_E2E_STEPS; round++) {
      if (!signedIn()) return error("account_cancelled");
      if (step.keystore !== saved) {
        host.saveKeystore(step.keystore);
        saved = step.keystore;
      }
      const request: ClipboardDriverRequest | undefined = step.request;
      if (request === undefined) {
        return step.error !== undefined ? error(step.error) : success(step.done);
      }
      const reply: AuthorizedReply = await this.authorizedResponse(
        request.method,
        request.path,
        request.body,
        undefined,
        undefined,
        sessionGuard,
      );
      if (reply.response === undefined) return error(reply.error ?? "account_unavailable");
      const body: string = reply.response.body;
      if (utf8Length(body) > MAX_CLIPBOARD_RESPONSE_BYTES) return error("account_unavailable");
      step = parseClipboardDriverStep(
        host.drive(
          JSON.stringify({ resume: step.state, response: { status: reply.response.status, body } }),
        ),
      );
    }
    return error("account_unavailable");
  }

  /**
   * 发送一条应用内反馈：`{type, text, app_version, edition, diagnostics}`，其中 `type` 取 `bug`、`suggestion` 或 `dictionary`，`text` 最多 500 个字符，`diagnostics` 为 null 或字符串字段的映射。请求体与 Android 的 `FeedbackApi` 载荷一致（`platform` 填本宿主的），服务端的应答不往上传：页面只需知道反馈已被接受。
   *
   * 有账号会话时附带它，无人登录时附带设备的匿名账号，与 Android 的 `FeedbackApi` 一致。只有两者都没有的设备（匿名注册还没成功）会以 `account_unauthorized` 拒绝，页面随之提供其他反馈渠道。
   */
  private async feedback(action: Action): Promise<string> {
    const diagnostics = feedbackDiagnostics(action.diagnostics);
    if (
      typeof action.type !== "string" ||
      !FEEDBACK_TYPES.includes(action.type) ||
      !validFeedbackText(action.text) ||
      !validString(action.app_version, MAX_FEEDBACK_VERSION_BYTES, true) ||
      !validString(action.edition, MAX_FEEDBACK_VERSION_BYTES, true) ||
      diagnostics === undefined
    ) {
      return error("account_invalid");
    }
    const body: Record<string, unknown> = {
      type: action.type,
      text: action.text.trim(),
      app_version: action.app_version,
      edition: action.edition,
      platform: "harmony",
    };
    if (diagnostics !== null && Object.keys(diagnostics).length > 0) body.diagnostics = diagnostics;
    const sender: AccountCloudBridge = this.accountOrAnonymous();
    const result: AuthorizedReply = await sender.authorizedResponse("POST", FEEDBACK_PATH, body);
    if (result.response === undefined) return error(result.error ?? "account_unavailable");
    const status: number = result.response.status;
    return status >= 200 && status < 300 ? success({}) : error(mapStatus(status));
  }

  /**
   * 「绑定 QQ 群助手」：列出本人的绑定，新的在前。回答只有编号和绑定时间，与桌面 `account_qq_links` 的形状相同。
   */
  private async qqLinks(): Promise<string> {
    const result = await this.authenticatedJson("GET", QQ_LINKS_PATH);
    if (result.value === undefined) return error(result.error ?? "account_unavailable");
    const links: unknown = result.value.links;
    if (!Array.isArray(links) || links.length > MAX_QQ_LINKS) return error("account_unavailable");
    const parsed: QqLink[] = [];
    for (const item of links) {
      const link = qqLink(item);
      if (link === null) return error("account_unavailable");
      parsed.push(link);
    }
    return success({ links: parsed });
  }

  /**
   * 用群友在 QQ 群里 @机器人 发 /bind 拿到的 8 位绑定码绑定。前后的空白和中间的空格、连字符先去掉，不是 8 位数字时不联网。码不存在、过期或用过是 `account_qq_link_code_not_found`，账号已经绑了 10 个群友是 `account_qq_link_limit`，与桌面的错误码相同，页面据此写说法。
   */
  private async qqLinkRedeem(action: Action): Promise<string> {
    const code = typeof action.code === "string" ? action.code.replace(/[\s-]/g, "") : "";
    if (!QQ_LINK_CODE.test(code)) return error("account_invalid");
    const result = await this.authorizedResponse("POST", QQ_LINKS_PATH, { code });
    if (result.response === undefined) return error(result.error ?? "account_unavailable");
    const response: AccountTransportResponse = result.response;
    const refusal = errorCode(response.body);
    if (response.status === 404 && refusal === "qq_link_code_not_found") {
      return error("account_qq_link_code_not_found");
    }
    if (response.status === 409 && refusal === "qq_link_limit") return error("account_qq_link_limit");
    if (response.status < 200 || response.status >= 300) return error(mapStatus(response.status));
    const link = qqLink(parseJson(response.body));
    return link === null ? error("account_unavailable") : success(link);
  }

  /** 解除本人的一条绑定；别人的或不存在的是 `account_not_found`，与桌面相同。 */
  private async qqLinkDelete(action: Action): Promise<string> {
    if (typeof action.id !== "string" || !QQ_LINK_ID.test(action.id)) return error("account_invalid");
    const result = await this.authorizedResponse("DELETE", `${QQ_LINKS_PATH}/${action.id}`);
    if (result.response === undefined) return error(result.error ?? "account_unavailable");
    const status: number = result.response.status;
    if (status === 404) return error("account_not_found");
    return status >= 200 && status < 300 ? success(null) : error(mapStatus(status));
  }

  /**
   * 打字统计「上传到云端」：把本机统计存储交回的请求体发出去，服务端回 204 时成功。只用登录账号，不退回匿名账号：没登录时是 `account_unauthorized`。调用方（设置页的串行队列）在成功后告诉存储记下上传时间。
   */
  async typingStatisticsUpload(body: unknown): Promise<string> {
    if (
      body === null ||
      typeof body !== "object" ||
      Array.isArray(body) ||
      utf8Length(JSON.stringify(body)) > MAX_TYPING_UPLOAD_BYTES
    ) {
      return error("account_invalid");
    }
    return await this.typingStatisticsRequest("PUT", body as Record<string, unknown>);
  }

  /** 关掉上传后删除云端的打字统计；没有数据时服务端也回 204。 */
  async typingStatisticsDelete(): Promise<string> {
    return await this.typingStatisticsRequest("DELETE");
  }

  private async typingStatisticsRequest(
    method: string,
    body?: Record<string, unknown>,
  ): Promise<string> {
    try {
      const result = await this.authorizedResponse(method, TYPING_STATISTICS_PATH, body);
      if (result.response === undefined) return error(result.error ?? "account_unavailable");
      const status: number = result.response.status;
      return status >= 200 && status < 300 ? success({}) : error(mapStatus(status));
    } catch (cause) {
      // 与 handle 相同：传输层抛出的不是 Error 时是断网一类的失败。
      return error(cause instanceof Error ? cause.message : "account_network");
    }
  }

  /**
   * The account-backed assistant, which is a different service from the user-configured one.
   *
   * The credential is the account session the host already holds, so the page never sees a token:
   * it names an operation and gets back a model list or one reply. Both halves are validated here
   * rather than in the page, because the page is the one surface that is not part of the product on
   * every host — the same bounds have to hold for a keyboard asking the same questions.
   */
  private async chat(action: Action): Promise<string> {
    const operation = action.chat_operation;
    if (operation === "models") return await this.chatModels();
    if (operation === "complete") return await this.chatComplete(action);
    return error("account_invalid");
  }

  private async chatModels(): Promise<string> {
    const result = await this.authenticatedJson("GET", "/v1/models");
    if (result.error !== undefined) return error(result.error);
    const value = result.value;
    if (value === undefined || value === null) return error("account_unavailable");
    const data = value.data;
    if (
      !Array.isArray(data) ||
      data.length === 0 ||
      data.length > MAX_CHAT_MODELS ||
      !validString(value.default_model, MAX_CHAT_MODEL_ID_BYTES)
    ) {
      return error("account_unavailable");
    }
    const ids: string[] = [];
    for (const model of data as unknown[]) {
      if (model === null || typeof model !== "object" || Array.isArray(model)) {
        return error("account_unavailable");
      }
      const id = (model as Action).id;
      if (
        !validString(id, MAX_CHAT_MODEL_ID_BYTES) ||
        !boundedUtf8(id, MAX_CHAT_MODEL_ID_BYTES) ||
        ids.includes(id)
      ) {
        return error("account_unavailable");
      }
      ids.push(id);
    }
    const defaultModel = value.default_model as string;
    if (!ids.includes(defaultModel)) return error("account_unavailable");
    return success({ data: ids.map((id) => ({ id })), defaultModel });
  }

  private async chatComplete(action: Action): Promise<string> {
    const model = action.model;
    const messages = action.messages;
    if (
      !validString(model, MAX_CHAT_MODEL_ID_BYTES) ||
      !boundedUtf8(model, MAX_CHAT_MODEL_ID_BYTES) ||
      !Array.isArray(messages) ||
      messages.length === 0 ||
      messages.length > MAX_CHAT_MESSAGES
    ) {
      return error("account_invalid");
    }
    const history: { role: string; content: string }[] = [];
    for (const message of messages as unknown[]) {
      if (message === null || typeof message !== "object" || Array.isArray(message)) {
        return error("account_invalid");
      }
      const role = (message as Action).role;
      const content = (message as Action).content;
      if (
        typeof role !== "string" ||
        !CHAT_ROLES.includes(role) ||
        !validChatText(content, MAX_CHAT_MESSAGE_BYTES)
      ) {
        return error("account_invalid");
      }
      history.push({ role, content });
    }
    const body: Record<string, unknown> = {
      messages: history,
      model,
      max_tokens: 2048,
      stream: false,
    };
    if (utf8Length(JSON.stringify(body)) > MAX_CHAT_REQUEST_BYTES) return error("account_invalid");
    const result = await this.authenticatedJson(
      "POST",
      "/v1/chat/completions",
      body,
      CHAT_TIMEOUT_MS,
    );
    if (result.error !== undefined) return error(result.error);
    if (result.value === undefined || result.value === null) return error("account_unavailable");
    const choices = result.value.choices;
    if (!Array.isArray(choices) || choices.length === 0) return error("account_unavailable");
    const first = choices[0] as unknown;
    if (first === null || typeof first !== "object" || Array.isArray(first)) {
      return error("account_unavailable");
    }
    const reply = (first as Action).message;
    if (reply === null || typeof reply !== "object" || Array.isArray(reply)) {
      return error("account_unavailable");
    }
    const role = (reply as Action).role;
    const content = (reply as Action).content;
    if (role !== "assistant" || !validChatText(content, MAX_CHAT_RESPONSE_BYTES)) {
      return error("account_unavailable");
    }
    return success({ content });
  }

  /**
   * The community skin gallery.
   *
   * Browsing is deliberately not authenticated: the gallery is public, and a signed-out user who
   * could not look at it would have no way to decide whether an account is worth making. The
   * session is attached when there is one, because that is what turns `owned` and `my_rating` into
   * this user's answers rather than nobody's. Everything that changes something — downloading,
   * rating, publishing, withdrawing — requires it.
   *
   * The design itself is not validated here beyond being an object. It is handed straight to the
   * shared store, which has the only complete definition of a valid design and normalizes it; a
   * second opinion written on this host would be the one that goes stale.
   */
  private async community(action: Action): Promise<string> {
    const operation = action.community_operation;
    if (operation === "list") {
      // 我的作品 is the one scope besides the public gallery; it is about the signed-in user, so it needs the session, and it asks for each item's moderation state so a removed skin can carry its 已下架 badge.
      const mine = action.scope === "mine";
      // 不传或为 null 时列出全部分类。
      const category = action.category ?? null;
      if (
        (action.scope !== undefined && action.scope !== "" && !mine) ||
        !this.boundedNumber(action.offset, 0, 1000000) ||
        !validString(action.search, MAX_COMMUNITY_SEARCH, true) ||
        (category !== null && !validCommunitySkinCategory(category))
      ) {
        return error("community_invalid");
      }
      const filter = category === null ? "" : `&category=${category}`;
      const path =
        `/v1/community/skins?${mine ? "scope=mine&fields=moderation&" : ""}offset=${action.offset}&q=` +
        `${encodeURIComponent(action.search)}${filter}&${INCLUDE_CATEGORY}`;
      return await this.communityRequest("GET", path, mine);
    }
    if (operation === "detail") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest(
        "GET",
        `/v1/community/skins/${action.id}?fields=moderation&${INCLUDE_CATEGORY}`,
        false,
      );
    }
    if (operation === "report") return await this.report("skins", action);
    if (operation === "download") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("POST", `/v1/community/skins/${action.id}/download`, true);
    }
    if (operation === "rate") {
      if (!validUuid(action.id) || !this.boundedNumber(action.stars, 1, 5)) {
        return error("community_invalid");
      }
      return await this.communityRequest("PUT", `/v1/community/skins/${action.id}/rating`, true, {
        stars: action.stars,
      });
    }
    if (operation === "publish") {
      const name = action.name;
      const description = action.description;
      if (
        !validUuid(action.id) ||
        !validCommunityText(name, 1, 32) ||
        name.trim() !== name ||
        !validCommunityText(description, 0, 280, true) ||
        !validSkinDesign(action.design) ||
        (action.category !== undefined && !validCommunitySkinCategory(action.category))
      ) {
        return error("community_invalid");
      }
      const body: Record<string, unknown> = {
        id: action.id,
        name,
        description,
        design: action.design,
      };
      // 不传分类时不发送该字段，由服务端归入默认分类。
      if (action.category !== undefined) body.category = action.category;
      return await this.communityRequest("POST", "/v1/community/skins", true, body);
    }
    if (operation === "set_category") {
      const id = action.id;
      const category = action.category;
      if (!validUuid(id) || !validCommunitySkinCategory(category)) {
        return error("community_invalid");
      }
      return await this.communityRequest(
        "PATCH",
        `/v1/community/skins/${id}?${INCLUDE_CATEGORY}`,
        true,
        { category },
        // 回显的分类不是请求的分类，说明修改没有生效。
        (value) => validCommunitySkinResponse(value, id) && value.category === category,
      );
    }
    if (operation === "unpublish") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("DELETE", `/v1/community/skins/${action.id}`, true);
    }
    return error("community_invalid");
  }

  /**
   * Reports another user's item to the moderators: `{id, reason, detail?}`, with `reason` one of `COMMUNITY_REPORT_REASONS`.
   *
   * Needs a session; the device's anonymous account counts. Reporting the same item again is accepted without a second record, so a retry is harmless.
   */
  private async report(kind: string, action: Action): Promise<string> {
    const detail = action.detail ?? "";
    if (
      !validUuid(action.id) ||
      typeof action.reason !== "string" ||
      !COMMUNITY_REPORT_REASONS.includes(action.reason) ||
      !validCommunityText(detail, 0, MAX_REPORT_DETAIL, true)
    ) {
      return error("community_invalid");
    }
    const body: Record<string, unknown> = { kind, item_id: action.id, reason: action.reason };
    if (detail.trim() !== "") body.detail = detail;
    return await this.communityRequest(
      "POST",
      "/v1/community/reports",
      true,
      body,
      (value) => value.reported === true,
    );
  }

  /**
   * One community request, with the session attached when there is one.
   *
   * The failure codes are the community vocabulary rather than the account one: the pages that read
   * these have wording for "已达到发布上限" and "自己的作品不能评分"，which an `account_conflict`
   * would arrive as the generic sentence instead.
   */
  private async communityRequest(
    method: string,
    path: string,
    authenticated: boolean,
    body?: Record<string, unknown>,
    responseValidator?: (value: Action) => boolean,
  ): Promise<string> {
    let response: AccountTransportResponse;
    if (authenticated) {
      const result: AuthorizedReply = await this.authorizedResponse(method, path, body);
      if (result.response === undefined) {
        return error(
          result.error === "account_cancelled"
            ? "community_cancelled"
            : result.error === "account_unauthorized"
              ? "community_unauthorized"
              : "community_unavailable",
        );
      }
      response = result.response;
    } else {
      const generation: number = this.generation;
      const currentToken: string | null = this.usableToken();
      const credential: CredentialReply =
        currentToken === null ? await this.credential() : { token: currentToken };
      const token: string | undefined = credential.token;
      response = await this.transport.request(method, path, token, body);
      if (generation !== this.generation && credential.error !== "account_unauthorized") {
        return error("community_cancelled");
      }
      // A public gallery stays public when an optional session expires. Retry without credentials
      // rather than turning a browse into a sign-in error.
      if ((response.status === 401 || response.status === 403) && token !== undefined) {
        response = await this.transport.request(method, path, undefined, body);
      }
    }
    if (response.status < 200 || response.status >= 300) {
      return error(
        communityRefusal(response.status, response.body) ?? communityStatus(response.status),
      );
    }
    const value = parseJson(response.body, communityResponseLimit(path));
    if (value === null && response.body.length > 0) return error("community_unavailable");
    if (
      method === "GET" &&
      path.startsWith("/v1/community/resources") &&
      !validateCommunityResourceResponse(path, value)
    ) {
      return error("community_unavailable");
    }
    if (
      method === "GET" &&
      path.startsWith("/v1/community/skins") &&
      !validateCommunitySkinResponse(path, value)
    ) {
      return error("community_unavailable");
    }
    if (responseValidator !== undefined && (value === null || !responseValidator(value))) {
      return error("community_unavailable");
    }
    if (path.startsWith("/v1/community/skins")) normalizeCommunitySkinCategories(value);
    return success(value ?? {});
  }

  /**
   * Community dictionaries and reply templates.
   *
   * The public list and one resource's detail are readable signed out, for the same reason the skin
   * gallery is. 我的作品 and 收藏 are not: they are questions about an account, and answering them
   * without one would either be empty or be somebody else's.
   */
  private async communityResource(action: Action): Promise<string> {
    const operation = action.resource_operation;
    if (operation === "list") {
      const kind = action.kind;
      const scope = action.scope;
      if (
        !validCommunityKind(kind) ||
        !validCommunityScope(scope) ||
        !this.boundedNumber(action.offset, 0, 1000000) ||
        !validString(action.search, MAX_COMMUNITY_SEARCH, true)
      ) {
        return error("community_invalid");
      }
      // A scope other than "all" is a question about the signed-in user, so it needs the session
      // rather than merely benefiting from it.
      // 我的作品 also asks for each item's moderation state, so a removed one can carry its 已下架 badge.
      const path =
        `/v1/community/resources?kind=${kind}&scope=${scope}` +
        `${scope === "mine" ? "&fields=moderation" : ""}` +
        `&q=${encodeURIComponent(action.search)}&offset=${action.offset}`;
      return await this.communityRequest("GET", path, scope !== "");
    }
    if (operation === "detail") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest(
        "GET",
        `/v1/community/resources/${action.id}?fields=moderation`,
        false,
      );
    }
    if (operation === "report") {
      if (!validCommunityKind(action.kind)) return error("community_invalid");
      return await this.report(action.kind === "dictionary" ? "dictionaries" : "replies", action);
    }
    if (operation === "publish") {
      const name = action.name;
      const description = action.description;
      if (
        !validUuid(action.id) ||
        !validCommunityKind(action.kind) ||
        !validCommunityText(name, 1, 32) ||
        name.trim() !== name ||
        !validCommunityText(description, 0, 280, true) ||
        !this.boundedNumber(action.revision, 0, 50000) ||
        !validResourceContent(action.kind, action.content)
      ) {
        return error("community_invalid");
      }
      return await this.communityRequest(
        "POST",
        "/v1/community/resources",
        true,
        {
          id: action.id,
          kind: action.kind,
          name,
          description,
          content: action.content,
          revision: action.revision,
        },
        (value) => validCommunityResourcePublication(value, action.id),
      );
    }
    if (operation === "apply") return await this.applyResource(action);
    if (operation === "save") {
      if (!validUuid(action.id) || typeof action.saved !== "boolean") {
        return error("community_invalid");
      }
      return await this.communityRequest(
        "PUT",
        `/v1/community/resources/${action.id}/save`,
        true,
        { saved: action.saved },
        (value) => validCommunitySaved(value, action.saved),
      );
    }
    if (operation === "rate") {
      if (!validUuid(action.id) || !this.boundedNumber(action.stars, 1, 5)) {
        return error("community_invalid");
      }
      return await this.communityRequest(
        "PUT",
        `/v1/community/resources/${action.id}/rating`,
        true,
        { stars: action.stars },
        (value) => validCommunityRating(value, action.stars),
      );
    }
    if (operation === "unpublish") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest(
        "DELETE",
        `/v1/community/resources/${action.id}`,
        true,
        undefined,
        validCommunityDeletion,
      );
    }
    return error("community_invalid");
  }

  /**
   * Merging a shared dictionary into the account's own.
   *
   * Two requests, because the server needs to be told which revision of the user's dictionary this
   * is being applied to: the read comes first and its answer goes into the write. The shared
   * service does the same, and it is the reason this is not a single call the page could make — a
   * page that held the revision between the two would be holding it across a screen the user can
   * leave.
   */
  private async applyResource(action: Action): Promise<string> {
    if (!validUuid(action.id) || !this.boundedNumber(action.resource_revision, 1, 50000)) {
      return error("community_invalid");
    }
    const catalog = await this.communityRequest(
      "GET",
      "/v1/users/me/dictionaries/quick/catalog?q=&offset=0&limit=100&scheme=pinyin&profile=xiaohe",
      true,
    );
    let dictionaryRevision: number;
    try {
      const parsed: Action = JSON.parse(catalog) as Action;
      if (parsed.ok !== true) return catalog;
      const value = parsed.value as Action;
      const revision = value.revision;
      if (!safeInteger(revision) || revision < 0) {
        return error("community_unavailable");
      }
      dictionaryRevision = revision;
    } catch {
      return error("community_unavailable");
    }
    return await this.communityRequest(
      "POST",
      `/v1/community/resources/${action.id}/apply`,
      true,
      {
        resource_revision: action.resource_revision,
        dictionary_revision: dictionaryRevision,
      },
      (value) =>
        validCommunityResourceApplication(value, action.resource_revision, dictionaryRevision),
    );
  }

  private kind(value: unknown): string | null {
    // 与共享 `DictionaryKind` 一致：98 五笔的云端词库是独立种类 `wubi98`。
    return typeof value === "string" &&
      ["pinyin", "wubi", "wubi98", "quick", "english"].includes(value)
      ? value
      : null;
  }

  private boundedNumber(value: unknown, minimum: number, maximum: number): value is number {
    return (
      typeof value === "number" && Number.isInteger(value) && value >= minimum && value <= maximum
    );
  }

  private async dictionary(action: Action): Promise<string> {
    const operation = action.dictionary_operation;
    const kind = this.kind(action.kind);
    if (operation === "list") {
      if (
        kind === null ||
        !validString(action.search, 1024, true) ||
        !this.boundedNumber(action.offset, 0, 1000000)
      )
        return error("account_invalid");
      return this.authenticated(
        "GET",
        `/v1/users/me/dictionaries/${kind}?q=${encodeURIComponent(action.search)}&offset=${action.offset}&limit=100`,
      );
    }
    if (operation === "catalog") {
      if (
        kind === null ||
        !validString(action.code, 256, true) ||
        !validString(action.scheme, 64) ||
        !validString(action.profile, 64) ||
        !this.boundedNumber(action.offset, 0, 1000000)
      )
        return error("account_invalid");
      return this.authenticated(
        "GET",
        `/v1/users/me/dictionaries/${kind}/catalog?q=${encodeURIComponent(action.code)}&offset=${action.offset}&limit=100&scheme=${encodeURIComponent(action.scheme)}&profile=${encodeURIComponent(action.profile)}`,
      );
    }
    if (operation === "add") {
      if (
        kind === null ||
        !validDictionaryValue(kind, action.code, action.word, action.weight, true)
      )
        return error("account_invalid");
      return this.authenticated("POST", `/v1/users/me/dictionaries/${kind}/add`, {
        code: action.code,
        word: action.word,
        weight: action.weight,
      });
    }
    if (operation === "update" || operation === "delete") {
      if (
        kind === null ||
        !validResourceId(action.id) ||
        !this.boundedNumber(action.revision, 1, 2147483647)
      )
        return error("account_invalid");
      const body =
        operation === "delete"
          ? { revision: action.revision }
          : {
              code: action.code,
              word: action.word,
              weight: action.weight,
              revision: action.revision,
            };
      if (
        operation === "update" &&
        !validDictionaryValue(kind, action.code, action.word, action.weight, true)
      )
        return error("account_invalid");
      return this.authenticated(
        operation === "delete" ? "DELETE" : "PUT",
        `/v1/users/me/dictionaries/${kind}/${action.id}`,
        body,
      );
    }
    if (operation === "edit_catalog") {
      if (
        kind === null ||
        !validDictionaryIdentity(kind, action.code, action.word) ||
        !this.boundedNumber(action.revision, 0, 2147483647)
      )
        return error("account_invalid");
      const replacement = action.replacement;
      if (
        replacement !== null &&
        (replacement === null ||
          typeof replacement !== "object" ||
          !validDictionaryValue(
            kind,
            (replacement as Action).code,
            (replacement as Action).word,
            (replacement as Action).weight,
            true,
          ))
      )
        return error("account_invalid");
      return this.authenticated("POST", `/v1/users/me/dictionaries/${kind}/edit`, {
        revision: action.revision,
        previous: { code: action.code, word: action.word },
        replacement,
      });
    }
    if (operation === "candidates") {
      if (
        !validString(action.text, 256) ||
        !validCandidateKind(action.kind) ||
        !validCandidateScheme(action.scheme) ||
        !validCandidateProfile(action.profile) ||
        !this.boundedNumber(action.limit, 1, 100)
      )
        return error("account_invalid");
      return this.authenticated("POST", "/v1/users/me/dictionary/candidates", {
        text: action.text,
        kind: action.kind,
        scheme: action.scheme,
        profile: action.profile,
        limit: action.limit,
      });
    }
    if (operation === "rank") {
      if (
        !validString(action.text, 256) ||
        !validCandidateKind(action.kind) ||
        !validCandidateScheme(action.scheme) ||
        !validCandidateProfile(action.profile) ||
        !this.boundedNumber(action.limit, 1, 100) ||
        !validCandidateValue(action.kind, action.code, action.word) ||
        action.kind === "quick" ||
        !this.boundedNumber(action.revision, 0, 2147483647) ||
        !validCandidateRankingMode(action.mode) ||
        !this.boundedNumber(action.linear_step, 1, 100) ||
        !this.boundedNumber(action.trigger_count, 1, 10) ||
        typeof action.force_top !== "boolean"
      )
        return error("account_invalid");
      return this.authenticated("POST", "/v1/users/me/dictionary/ranking", {
        revision: action.revision,
        query: {
          text: action.text,
          kind: action.kind,
          scheme: action.scheme,
          profile: action.profile,
          limit: action.limit,
        },
        action: {
          code: action.code,
          word: action.word,
          mode: action.mode,
          linear_step: action.linear_step,
          trigger_count: action.trigger_count,
          force_top: action.force_top,
        },
      });
    }
    if (operation === "remove_candidate") {
      if (
        !validString(action.text, 256) ||
        !validCandidateKind(action.kind) ||
        !validCandidateScheme(action.scheme) ||
        !validCandidateProfile(action.profile) ||
        !this.boundedNumber(action.limit, 1, 100) ||
        !validCandidateValue(action.kind, action.code, action.word) ||
        action.kind === "quick" ||
        !this.boundedNumber(action.revision, 0, 2147483647)
      )
        return error("account_invalid");
      return this.authenticated("DELETE", "/v1/users/me/dictionary/candidates", {
        revision: action.revision,
        query: {
          text: action.text,
          kind: action.kind,
          scheme: action.scheme,
          profile: action.profile,
          limit: action.limit,
        },
        code: action.code,
        word: action.word,
      });
    }
    if (operation === "fixed_positions") {
      if (
        !validString(action.context, 1024, true) ||
        !this.boundedNumber(action.offset, 0, 1000000)
      )
        return error("account_invalid");
      return this.authenticated(
        "GET",
        `/v1/users/me/dictionary/positions?context=${encodeURIComponent(action.context)}&offset=${action.offset}&limit=100`,
      );
    }
    if (operation === "set_fixed_position") {
      if (
        !validString(action.context, 1024, true) ||
        !validString(action.code, 256) ||
        !validString(action.word, 1024) ||
        !this.boundedNumber(action.revision, 0, 2147483647) ||
        (action.position !== null && !this.boundedNumber(action.position, 1, 5))
      )
        return error("account_invalid");
      const body: Record<string, unknown> = {
        context: action.context,
        code: action.code,
        word: action.word,
        revision: action.revision,
      };
      // DELETE means removing the fixed position. The service distinguishes an absent position
      // from a JSON null, so only PUT carries this field (the fixed Apple client does the same).
      if (action.position !== null) body.position = action.position;
      return this.authenticated(
        action.position === null ? "DELETE" : "PUT",
        "/v1/users/me/dictionary/positions",
        body,
      );
    }
    if (operation === "import") {
      if (
        kind === null ||
        !validString(action.format, 16) ||
        !["standard", "windows", "hans"].includes(action.format) ||
        (action.format === "hans" && kind !== "pinyin") ||
        !validString(action.text, 64 * 1024) ||
        !boundedUtf8(action.text, 64 * 1024)
      )
        return error("account_invalid");
      const path =
        action.format === "hans"
          ? `/v1/users/me/dictionaries/${kind}/import-hans`
          : `/v1/users/me/dictionaries/${kind}/import`;
      const body =
        action.format === "hans"
          ? { text: action.text, weight: 100000 }
          : { text: action.text, format: action.format };
      return this.authenticated("POST", path, body);
    }
    return error("account_invalid");
  }

  private async requestPublic(
    method: string,
    path: string,
    body?: Record<string, unknown>,
  ): Promise<string> {
    const response = await this.transport.request(method, path, undefined, body);
    return this.response(response);
  }

  /** Returns one current access token, sharing refresh-token rotation across concurrent callers. */
  private async credential(rejectedToken?: string): Promise<CredentialReply> {
    const usable: string | null = this.usableToken(rejectedToken);
    if (usable !== null) return { token: usable };
    if (this.session === null) return { error: "account_unauthorized" };
    if (this.refreshing !== null) return await this.refreshing;
    const generation = this.generation;
    const flight: Promise<CredentialReply> = this.exclusiveRefresh(generation, rejectedToken);
    this.refreshing = flight;
    try {
      return await flight;
    } finally {
      if (this.refreshing === flight) this.refreshing = null;
    }
  }

  /**
   * Refresh while holding the store's cross-process lock, deciding from the session on disk rather than the one in memory.
   *
   * The settings application and the keyboard extension run in different processes over the one session file. The service rotates the refresh token on every refresh and revokes the whole session when a spent one is presented, so two processes refreshing the same token, or one refreshing a token the other has already rotated, signs the user out everywhere. Inside the lock this re-reads the store and takes up a rotation the other process saved; only when the stored access token is no better than this one's does it refresh, and the rotation is saved before the lock is released.
   */
  private async exclusiveRefresh(
    generation: number,
    rejectedToken?: string,
  ): Promise<CredentialReply> {
    try {
      return await this.locked(
        async (): Promise<CredentialReply> => await this.refreshLatest(generation, rejectedToken),
      );
    } catch {
      // Without the lock there is no knowing the other process is not refreshing this token right now, and guessing wrong revokes the session; leaving it unrefreshed is the safe failure.
      return { error: "account_unavailable" };
    }
  }

  private async refreshLatest(
    generation: number,
    rejectedToken?: string,
  ): Promise<CredentialReply> {
    if (generation !== this.generation) return { error: "account_cancelled" };
    // The settings page signed in as someone else since this bridge read the file. Refreshing the old account's token would be refused and the refusal would clear the new account's session.
    if (this.storedUserChanged()) return { error: "account_cancelled" };
    this.adoptStored();
    const usable: string | null = this.usableToken(rejectedToken);
    if (usable !== null) return { token: usable };
    const current: Session | null = this.session;
    if (current === null) return { error: "account_unauthorized" };
    return await this.refresh(current, generation);
  }

  /**
   * Take up a rotation another process saved for this account since this one last read the store.
   *
   * Only a valid session for the same user that expires later than the one in memory is newer: each rotation moves the expiry forward, so an older document left behind by a failed write is never mistaken for one. A sign-out or a different account is not picked up here; those are the settings page's own operations and arrive through them.
   */
  private adoptStored(): void {
    const current: Session | null = this.session;
    const stored: Session | null = this.storedSession();
    if (
      current !== null &&
      stored !== null &&
      stored.user.id === current.user.id &&
      stored.refresh_token !== current.refresh_token &&
      stored.expires_at > current.expires_at
    ) {
      this.adoptedStoredSession = true;
      this.session = stored;
    }
  }

  /** Whether the store now holds a valid session for a different account from the one in memory. */
  private storedUserChanged(): boolean {
    const current: Session | null = this.session;
    const stored: Session | null = this.storedSession();
    return current !== null && stored !== null && stored.user.id !== current.user.id;
  }

  private storedSession(): Session | null {
    const saved: string | null = this.store.load();
    if (saved === null || utf8Length(saved) > MAX_SESSION_BYTES) return null;
    try {
      const value: unknown = JSON.parse(saved);
      return validateSession(value) ? value : null;
    } catch {
      return null;
    }
  }

  private usableToken(rejectedToken?: string): string | null {
    const current: Session | null = this.session;
    if (
      current === null ||
      current.expires_at <= Date.now() + 30000 ||
      rejectedToken === current.access_token
    )
      return null;
    return current.access_token;
  }

  /**
   * Spend `from`'s refresh token. Callers hold the store's lock.
   *
   * The store is read again right before anything is written. Unlocked writers remain — a sign-out that could not take the lock, a document refused as corrupt — and either can land while the request is out; the rotation is then not written over whatever they left.
   */
  private async refresh(from: Session, generation: number): Promise<CredentialReply> {
    const refreshToken: string = from.refresh_token;
    let response: AccountTransportResponse;
    try {
      response = await this.transport.request("POST", "/v1/auth/refresh", undefined, {
        refresh_token: refreshToken,
      });
    } catch {
      return { error: "account_unavailable" };
    }
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status === 401 || response.status === 403) {
      this.expireSession((session: Session): boolean => session.refresh_token === refreshToken);
      return { error: "account_unauthorized" };
    }
    if (response.status < 200 || response.status >= 300) {
      return { error: mapStatus(response.status) };
    }
    const value = parseJson(response.body);
    if (value === null) return { error: "account_unavailable" };
    const next: Session | null = sessionFromTokens(value);
    if (next === null) return { error: "account_unavailable" };
    if (generation !== this.generation) return { error: "account_cancelled" };
    const stored: Session | null = this.storedSession();
    if (stored === null) {
      // Signed out while the request was out: the rotation belongs to a session the user ended.
      this.forgetSession();
      return { error: "account_unauthorized" };
    }
    if (stored.user.id !== next.user.id) {
      this.forgetSession();
      return { error: "account_cancelled" };
    }
    if (stored.refresh_token !== refreshToken && stored.expires_at > from.expires_at) {
      // A newer session for this account was saved meanwhile, a fresh sign-in; it stays, and this process uses it.
      this.adoptedStoredSession = true;
      this.session = stored;
      return { token: stored.access_token };
    }
    // The service has already spent the old token, so memory takes the rotation even if the write below fails; refreshing the spent one again would revoke the session.
    this.session = next;
    this.store.save(JSON.stringify(next));
    return { token: next.access_token };
  }

  /**
   * 接受任一种账号的请求所使用的会话属于哪个 bridge：有人登录时用本 bridge，否则匿名账号有会话时用匿名账号的，再否则仍用本 bridge，由它以 `account_unauthorized` 拒绝。
   *
   * 在匿名 bridge 拿到会话之前，每次都从它的 store 重新读取，因为首次启动时原生注册与设置页并行进行，可能在本 bridge 创建之后才完成。
   */
  private accountOrAnonymous(): AccountCloudBridge {
    if (this.session !== null || this.anonymousStore === undefined) return this;
    if (this.anonymous === null || this.anonymous.session === null) {
      this.anonymous = new AccountCloudBridge(this.transport, this.anonymousStore);
    }
    return this.anonymous.session === null ? this : this.anonymous;
  }

  /** Sends once with current credentials and retries one rejected token after rotation. */
  private async authorizedResponse(
    method: string,
    path: string,
    body?: Record<string, unknown>,
    timeoutMs?: number,
    requestTag?: string,
    sessionGuard?: () => boolean,
  ): Promise<AuthorizedReply> {
    const currentToken: string | null = this.usableToken();
    let credential: CredentialReply =
      currentToken === null ? await this.credential() : { token: currentToken };
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    if (sessionGuard !== undefined && !sessionGuard()) return { error: "account_cancelled" };
    let token: string = credential.token;
    let generation: number = this.generation;
    let response: AccountTransportResponse = await this.transport.request(
      method,
      path,
      token,
      body,
      timeoutMs,
      requestTag,
    );
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status !== 401 && response.status !== 403) return { response, token };
    // A banned account is refused for what it is, not for a stale token: refreshing would only be refused again, and the caller needs the reason.
    // 云剪贴板关闭时服务端以 403 `clipboard_sync_disabled` 拒绝写入，同样与令牌无关：当成令牌问题去刷新，第二次 403 会把用户登出。
    if (
      response.status === 403 &&
      FORBIDDEN_NOT_CREDENTIAL.includes(errorCode(response.body) ?? "")
    ) {
      return { response, token };
    }
    credential = await this.credential(token);
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    if (sessionGuard !== undefined && !sessionGuard()) return { error: "account_cancelled" };
    token = credential.token;
    generation = this.generation;
    response = await this.transport.request(method, path, token, body, timeoutMs, requestTag);
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status === 401 || response.status === 403) {
      await this.expireAccessToken(token);
      return { error: "account_unauthorized" };
    }
    return { response, token };
  }

  /** The file equivalent of `authorizedResponse`, including refresh and generation invalidation. */
  private async authorizedDownload(
    path: string,
    destination: string,
    maximumBytes: number,
    mediaType: string,
  ): Promise<{ response?: AccountDownloadResponse; error?: string }> {
    const download = this.transport.download;
    if (download === undefined) return { error: "account_unavailable" };
    const currentToken: string | null = this.usableToken();
    let credential: CredentialReply =
      currentToken === null ? await this.credential() : { token: currentToken };
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    let token: string = credential.token;
    let generation: number = this.generation;
    let response: AccountDownloadResponse = await download.call(
      this.transport,
      path,
      token,
      destination,
      maximumBytes,
      mediaType,
    );
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status !== 401 && response.status !== 403) return { response };
    credential = await this.credential(token);
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    token = credential.token;
    generation = this.generation;
    response = await download.call(
      this.transport,
      path,
      token,
      destination,
      maximumBytes,
      mediaType,
    );
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status === 401 || response.status === 403) {
      await this.expireAccessToken(token);
      return { error: "account_unauthorized" };
    }
    return { response };
  }

  /** The snapshot-upload equivalent of `authorizedResponse`, including one token rotation. */
  private async authorizedSnapshotUpload(
    source: string,
    revision: number,
    expectedSha256: string,
  ): Promise<AuthorizedReply> {
    const upload = this.transport.uploadSnapshot;
    if (upload === undefined) return { error: "account_unavailable" };
    const currentToken: string | null = this.usableToken();
    let credential: CredentialReply =
      currentToken === null ? await this.credential() : { token: currentToken };
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    let token: string = credential.token;
    let generation: number = this.generation;
    let response = await upload.call(this.transport, source, revision, expectedSha256, token);
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status !== 401 && response.status !== 403) return { response };
    credential = await this.credential(token);
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    token = credential.token;
    generation = this.generation;
    response = await upload.call(this.transport, source, revision, expectedSha256, token);
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status === 401 || response.status === 403) {
      await this.expireAccessToken(token);
      return { error: "account_unauthorized" };
    }
    return { response };
  }

  private async authenticated(
    method: string,
    path: string,
    body?: Record<string, unknown>,
  ): Promise<string> {
    const result = await this.authorizedResponse(method, path, body);
    if (result.response === undefined) return error(result.error ?? "account_unavailable");
    return this.response(result.response);
  }

  /**
   * An authenticated request whose body the caller validates itself.
   *
   * `authenticated` answers the page directly, which is right for the endpoints whose response is
   * already the DTO. Chat is not one of them: its reply has to be checked and reshaped before the
   * page sees it, so this returns the parsed document and leaves the envelope to the caller. The
   * session, expiry and generation handling is the same either way — that part must not be
   * duplicated, or one of the two copies eventually stops clearing an expired session.
   */
  private async authenticatedJson(
    method: string,
    path: string,
    body?: Record<string, unknown>,
    timeoutMs?: number,
    requestTag?: string,
  ): Promise<{ value?: Action; error?: string }> {
    const result = await this.authorizedResponse(method, path, body, timeoutMs, requestTag);
    if (result.response === undefined) return { error: result.error ?? "account_unavailable" };
    const response: AccountTransportResponse = result.response;
    if (response.status < 200 || response.status >= 300)
      return { error: mapStatus(response.status) };
    const value = parseJson(response.body);
    if (value === null) return { error: "account_unavailable" };
    return { value };
  }

  private response(response: AccountTransportResponse): string {
    if (response.status < 200 || response.status >= 300) return error(mapStatus(response.status));
    const value = parseJson(response.body);
    return value === null && response.body.length > 0
      ? error("account_unavailable")
      : success(value ?? {});
  }

  /** Runs `body` under the store's cross-process lock when it has one; rejects when the lock cannot be taken. */
  private async locked<T>(body: () => Promise<T>): Promise<T> {
    const store: AccountSessionStore = this.store;
    return store.exclusive === undefined ? await body() : await store.exclusive<T>(body);
  }

  /**
   * The user signed out or deleted the account: forget the session and clear the store, under the lock so a refresh in the other process cannot write it back.
   *
   * The user asked for this, so a lock that cannot be taken does not keep them signed in; the refresh path re-reads the store before writing and leaves an emptied one alone.
   */
  private async signOut(): Promise<void> {
    try {
      await this.locked(async (): Promise<void> => this.clearExpired());
    } catch {
      this.clearExpired();
    }
  }

  /** The service refused this access token even after a refresh; forget its session, and clear the store only if the store still holds it. */
  private async expireAccessToken(token: string): Promise<void> {
    const matches = (session: Session): boolean => session.access_token === token;
    try {
      await this.locked(async (): Promise<void> => this.expireSession(matches));
    } catch {
      // Without the lock the store is left as it is; this process still stops using the refused session.
      if (this.session !== null && matches(this.session)) this.forgetSession();
    }
  }

  /**
   * Forget a session the service refused. Callers hold the lock.
   *
   * The store is cleared only if it still holds that session: a sign-in saved since the refused request went out belongs to a session the refusal says nothing about.
   */
  private expireSession(matches: (session: Session) => boolean): void {
    if (this.session !== null && matches(this.session)) this.forgetSession();
    const stored: Session | null = this.storedSession();
    if (stored !== null && matches(stored)) this.clearStoredSession(stored);
  }

  private clearStoredSession(stored: Session | null): void {
    if (stored !== null) this.store.beforeClear?.(stored.user.id);
    this.discardStoredSession();
  }

  /** 删除保存的会话，云剪贴板的密钥串随它一起删：退出登录、令牌被拒和会话文件损坏都走这里。 */
  private discardStoredSession(): void {
    this.store.clear();
    this.clipboardE2eHost?.clearKeystore();
  }

  private forgetSession(): void {
    this.session = null;
    this.generation++;
    this.refreshing = null;
    this.transport.cancelDownloads?.();
  }

  private clearExpired(): void {
    this.forgetSession();
    this.clearStoredSession(this.storedSession());
  }
}
