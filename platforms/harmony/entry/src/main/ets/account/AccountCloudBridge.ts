import { utf8Length } from "../keyboard/Utf8";
import { CustomKeyboardSkin, CustomSkinDocument } from "../keyboard/skin/CustomKeyboardSkin";

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
const MAX_CLIPBOARD_TEXT = 4000;
const MAX_SEARCH = 256;

/**
 * The chat bounds, which are the shared ones rather than a HarmonyOS reading of them.
 *
 * They match `BackendChatClient` on Apple and `client-core`'s account client byte for byte, because
 * the server enforces the same numbers and a client that is stricter turns a working conversation
 * into an unexplained refusal on one platform only.
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

/** The gallery's own bounds, matching `client-core`'s community skin service. */
const MAX_COMMUNITY_SEARCH = 128;
/** Only the legacy JSON/WebView fallback is capped at 3 MiB; native files use the source limits. */
export const MAX_BRIDGED_DICTIONARY_EXPORT_BYTES = 3 * 1024 * 1024;
export const MAX_DICTIONARY_EXPORT_BYTES = 384 * 1024 * 1024;
export const MAX_SNAPSHOT_DOWNLOAD_BYTES = 512 * 1024 * 1024;

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
  const characters = [...value];
  if (characters.length < minimum || characters.length > maximum) return false;
  return !characters.some((character) => {
    if (multiline && (character === "\n" || character === "\t")) return false;
    const code = character.codePointAt(0) ?? 0;
    return code <= 0x1f || code === 0x7f;
  });
}

function validCommunityKind(value: unknown): value is string {
  return value === "dictionary" || value === "reply";
}

/** `""` is 全部, and the only scope a signed-out browser can ask for. */
function validCommunityScope(value: unknown): value is string {
  return value === "" || value === "mine" || value === "saved";
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
      !["pinyin", "wubi", "quick", "english"].includes(entryKind) ||
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
  return [...text.trim()].length >= minimum;
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
    return current === undefined || (allowNull && current === null) ||
      (typeof current === "number" && safeInteger(current) && current >= 0 && current <= 0xffffff);
  };
  const boundedNumber = (field: string, minimum: number, maximum: number, allowNull = false): boolean => {
    const current = design[field];
    return current === undefined || (allowNull && current === null) ||
      (typeof current === "number" && Number.isFinite(current) && current >= minimum && current <= maximum);
  };
  if (
    !["background", "keyBackground", "keyForeground", "accent", "actionBackground"].every((field) => color(field)) ||
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
  if (design.gradientHorizontal !== undefined && typeof design.gradientHorizontal !== "boolean") return false;
  if (
    (design.keyShape !== undefined && design.keyShape !== null &&
      !["rounded", "capsule", "ticket", "pebble"].includes(design.keyShape as string)) ||
    (design.keyMaterial !== undefined && design.keyMaterial !== null &&
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
    (skin.rating_count !== 0 || skin.rating_average === 0)
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
    ) return false;
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
    const id = path.slice(detailPrefix.length);
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
    !validResourceContent(resource.kind, resource.content)
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
    const id = path.slice(detailPrefix.length);
    return validResourceUuid(id) && validCommunityResourceResponse(value, undefined, id);
  }
  return true;
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

function validString(value: unknown, maximum: number, allowEmpty = false): value is string {
  return (
    typeof value === "string" &&
    (allowEmpty || value.length > 0) &&
    value.length <= maximum &&
    ![...value].some((character) => {
      const code = character.codePointAt(0) ?? 0;
      return code <= 0x1f || code === 0x7f;
    })
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
    typeof session.expires_at === "number" &&
    Number.isFinite(session.expires_at) &&
    session.expires_at <= Date.now() + MAX_SESSION_MILLISECONDS &&
    validateUser(session.user)
  );
}

function sessionFromTokens(value: Action): Session | null {
  if (
    !validToken(value.access_token) ||
    !validToken(value.refresh_token) ||
    value.token_type !== "Bearer" ||
    typeof value.expires_in !== "number" ||
    !Number.isFinite(value.expires_in) ||
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

  constructor(transport: AccountTransport, store: AccountSessionStore) {
    this.transport = transport;
    this.store = store;
    const saved = store.load();
    if (saved !== null) {
      try {
        const value: unknown = JSON.parse(saved);
        if (validateSession(value)) this.session = value;
        else store.clear();
      } catch {
        store.clear();
      }
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
        case "profile":
          return await this.profile();
        case "rename":
          return await this.rename(action);
        case "logout":
          return await this.logout(action);
        case "delete_account":
          return await this.deleteAccount();
        case "clear_expired":
          this.clearExpired();
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
        default:
          return error("account_invalid");
      }
    } catch (cause) {
      // The community pages decode a different vocabulary from the account ones, so a thrown
      // transport failure has to arrive in the one its caller has wording for.
      if (operation === "community_skin" || operation === "community_resource") {
        return error("community_unavailable");
      }
      return error(cause instanceof Error ? cause.message : "account_unavailable");
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
    if (result.error !== undefined || result.value === undefined)
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

  /** Native hosts save exports themselves so multi-megabyte text never crosses a WebView bridge. */
  async downloadDictionary(
    kind: unknown,
    format: unknown,
  ): Promise<{ body?: string; error?: string }> {
    const acceptedKind = this.kind(kind);
    if (
      acceptedKind === null ||
      typeof format !== "string" ||
      !["standard", "windows"].includes(format)
    )
      return { error: "account_invalid" };
    const result = await this.authorizedResponse(
      "GET",
      `/v1/users/me/dictionaries/${acceptedKind}/export?format=${format}`,
    );
    if (result.response === undefined) return { error: result.error ?? "account_unavailable" };
    const response = result.response;
    if (response.status < 200 || response.status >= 300)
      return { error: mapStatus(response.status) };
    const bytes = utf8Length(response.body);
    if (
      bytes === 0 ||
      bytes > MAX_BRIDGED_DICTIONARY_EXPORT_BYTES ||
      response.body.includes("\u0000") ||
      (response.contentLength !== undefined && response.contentLength !== bytes)
    )
      return { error: "account_unavailable" };
    return { body: response.body };
  }

  private async requestCode(action: Action): Promise<string> {
    const provider = action.provider;
    const target = action.target;
    if (
      !validString(provider, 16) ||
      !["email", "phone"].includes(provider) ||
      !validString(target, 320) ||
      target.trim() !== target
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
    this.generation++;
    this.refreshing = null;
    const generation = this.generation;
    const response = await this.transport.request("POST", "/v1/auth/login", undefined, {
      challenge_id: action.challenge_id,
      credential: action.credential,
    });
    if (generation !== this.generation) return error("account_cancelled");
    if (response.status < 200 || response.status >= 300) return error(mapStatus(response.status));
    const value = parseJson(response.body);
    if (value === null) return error("account_unavailable");
    const session: Session | null = sessionFromTokens(value);
    if (session === null) return error("account_unavailable");
    this.session = session;
    this.store.save(JSON.stringify(session));
    return success({ user: session.user });
  }

  private async rename(action: Action): Promise<string> {
    if (
      !validString(action.display_name, 256) ||
      [...action.display_name].length > 64 ||
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
    const updated: Session = { ...current, user: value.user };
    this.store.save(JSON.stringify(updated));
    this.session = updated;
    return success(value);
  }

  private async logout(action: Action): Promise<string> {
    if (typeof action.all !== "boolean") return error("account_invalid");
    const result = await this.authenticated("POST", "/v1/auth/logout", { all: action.all });
    if (JSON.parse(result).ok) this.clearExpired();
    return result;
  }

  private async deleteAccount(): Promise<string> {
    const result = await this.authenticated("DELETE", "/v1/users/me");
    if (JSON.parse(result).ok) this.clearExpired();
    return result;
  }

  private async clipboard(action: Action): Promise<string> {
    const operation = action.clipboard_operation;
    if (operation === "list") {
      if (!validString(action.search, MAX_SEARCH, true)) return error("account_invalid");
      return this.authenticated(
        "GET",
        `/v1/users/me/clipboard?q=${encodeURIComponent(action.search)}`,
      );
    }
    if (operation === "add") {
      if (!validString(action.text, MAX_CLIPBOARD_TEXT) || action.text.trim().length === 0)
        return error("account_invalid");
      return this.authenticated("POST", "/v1/users/me/clipboard", { text: action.text });
    }
    if (operation === "delete") {
      if (!validResourceId(action.id)) return error("account_invalid");
      return this.authenticated("DELETE", `/v1/users/me/clipboard/${action.id}`);
    }
    if (operation === "set_enabled") {
      if (typeof action.enabled !== "boolean") return error("account_invalid");
      const result = await this.authenticated("PUT", "/v1/users/me/clipboard/settings", {
        enabled: action.enabled,
      });
      return JSON.parse(result).ok ? success({ enabled: action.enabled }) : result;
    }
    return error("account_invalid");
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
    if (value === undefined) return error("account_unavailable");
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
      // Newlines are content here, not a control character to refuse: a conversation is written in
      // paragraphs, and the shared clients bound the text by bytes rather than by character class.
      if (
        typeof role !== "string" ||
        !CHAT_ROLES.includes(role) ||
        typeof content !== "string" ||
        content.length === 0 ||
        !boundedUtf8(content, MAX_CHAT_MESSAGE_BYTES)
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
    if (result.value === undefined) return error("account_unavailable");
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
    if (
      role !== "assistant" ||
      typeof content !== "string" ||
      content.trim().length === 0 ||
      !boundedUtf8(content, MAX_CHAT_RESPONSE_BYTES)
    ) {
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
      if (
        !this.boundedNumber(action.offset, 0, 1000000) ||
        !validString(action.search, MAX_COMMUNITY_SEARCH, true)
      ) {
        return error("community_invalid");
      }
      const path =
        `/v1/community/skins?offset=${action.offset}&q=` + `${encodeURIComponent(action.search)}`;
      return await this.communityRequest("GET", path, false);
    }
    if (operation === "detail") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("GET", `/v1/community/skins/${action.id}`, false);
    }
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
        action.design === null ||
        typeof action.design !== "object" ||
        Array.isArray(action.design)
      ) {
        return error("community_invalid");
      }
      return await this.communityRequest("POST", "/v1/community/skins", true, {
        id: action.id,
        name,
        description,
        design: action.design,
      });
    }
    if (operation === "unpublish") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("DELETE", `/v1/community/skins/${action.id}`, true);
    }
    return error("community_invalid");
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
      return error(communityStatus(response.status));
    }
    const value = parseJson(response.body, communityResponseLimit(path));
    if (value === null && response.body.length > 0) return error("community_unavailable");
    if (method === "GET" && path.startsWith("/v1/community/resources") &&
        !validateCommunityResourceResponse(path, value)) {
      return error("community_unavailable");
    }
    if (method === "GET" && path.startsWith("/v1/community/skins") &&
        !validateCommunitySkinResponse(path, value)) {
      return error("community_unavailable");
    }
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
      const path =
        `/v1/community/resources?kind=${kind}&scope=${scope}` +
        `&q=${encodeURIComponent(action.search)}&offset=${action.offset}`;
      return await this.communityRequest("GET", path, scope !== "");
    }
    if (operation === "detail") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("GET", `/v1/community/resources/${action.id}`, false);
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
      return await this.communityRequest("POST", "/v1/community/resources", true, {
        id: action.id,
        kind: action.kind,
        name,
        description,
        content: action.content,
        revision: action.revision,
      });
    }
    if (operation === "apply") return await this.applyResource(action);
    if (operation === "save") {
      if (!validUuid(action.id) || typeof action.saved !== "boolean") {
        return error("community_invalid");
      }
      return await this.communityRequest("PUT", `/v1/community/resources/${action.id}/save`, true, {
        saved: action.saved,
      });
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
      );
    }
    if (operation === "unpublish") {
      if (!validUuid(action.id)) return error("community_invalid");
      return await this.communityRequest("DELETE", `/v1/community/resources/${action.id}`, true);
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
      if (typeof revision !== "number" || !Number.isInteger(revision) || revision < 0) {
        return error("community_unavailable");
      }
      dictionaryRevision = revision;
    } catch {
      return error("community_unavailable");
    }
    return await this.communityRequest("POST", `/v1/community/resources/${action.id}/apply`, true, {
      resource_revision: action.resource_revision,
      dictionary_revision: dictionaryRevision,
    });
  }

  private kind(value: unknown): string | null {
    return typeof value === "string" && ["pinyin", "wubi", "quick", "english"].includes(value)
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
        !validString(action.code, 256) ||
        !validString(action.word, 1024) ||
        !this.boundedNumber(action.weight, 0, 2147483647)
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
        (!validString(action.code, 256) ||
          !validString(action.word, 1024) ||
          !this.boundedNumber(action.weight, 0, 2147483647))
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
        !validString(action.code, 256) ||
        !validString(action.word, 1024) ||
        !this.boundedNumber(action.revision, 0, 2147483647)
      )
        return error("account_invalid");
      const replacement = action.replacement;
      if (
        replacement !== null &&
        (replacement === null ||
          typeof replacement !== "object" ||
          !validString((replacement as Action).code, 256) ||
          !validString((replacement as Action).word, 1024) ||
          !this.boundedNumber((replacement as Action).weight, 0, 2147483647))
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
        !validString(action.text, 1024) ||
        !validString(action.kind, 32) ||
        !validString(action.scheme, 64) ||
        !validString(action.profile, 64) ||
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
        !validString(action.text, 1024) ||
        !validString(action.kind, 32) ||
        !validString(action.scheme, 64) ||
        !validString(action.profile, 64) ||
        !this.boundedNumber(action.limit, 1, 100) ||
        !validString(action.code, 256) ||
        !validString(action.word, 1024) ||
        !this.boundedNumber(action.revision, 0, 2147483647) ||
        !validString(action.mode, 16) ||
        !this.boundedNumber(action.linear_step, 0, 100) ||
        !this.boundedNumber(action.trigger_count, 0, 100) ||
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
        !validString(action.text, 1024) ||
        !validString(action.kind, 32) ||
        !validString(action.scheme, 64) ||
        !validString(action.profile, 64) ||
        !this.boundedNumber(action.limit, 1, 100) ||
        !validString(action.code, 256) ||
        !validString(action.word, 1024) ||
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
    if (operation === "export") {
      const downloaded = await this.downloadDictionary(action.kind, action.format);
      if (downloaded.body === undefined) return error(downloaded.error ?? "account_unavailable");
      return success({
        text: downloaded.body,
        filename: `dictionary-${kind}.tsv`,
      });
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
    const current = this.session;
    if (current === null) return { error: "account_unauthorized" };
    if (this.refreshing !== null) return await this.refreshing;
    const generation = this.generation;
    const flight: Promise<CredentialReply> = this.refresh(current.refresh_token, generation);
    this.refreshing = flight;
    try {
      return await flight;
    } finally {
      if (this.refreshing === flight) this.refreshing = null;
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

  private async refresh(refreshToken: string, generation: number): Promise<CredentialReply> {
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
      this.clearExpired();
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
    this.store.save(JSON.stringify(next));
    this.session = next;
    return { token: next.access_token };
  }

  /** Sends once with current credentials and retries one rejected token after rotation. */
  private async authorizedResponse(
    method: string,
    path: string,
    body?: Record<string, unknown>,
    timeoutMs?: number,
    requestTag?: string,
  ): Promise<AuthorizedReply> {
    const currentToken: string | null = this.usableToken();
    let credential: CredentialReply =
      currentToken === null ? await this.credential() : { token: currentToken };
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
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
    credential = await this.credential(token);
    if (credential.token === undefined)
      return { error: credential.error ?? "account_unauthorized" };
    token = credential.token;
    generation = this.generation;
    response = await this.transport.request(method, path, token, body, timeoutMs, requestTag);
    if (generation !== this.generation) return { error: "account_cancelled" };
    if (response.status === 401 || response.status === 403) {
      this.clearExpired();
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
      this.clearExpired();
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
      this.clearExpired();
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

  private clearExpired(): void {
    this.session = null;
    this.generation++;
    this.refreshing = null;
    this.transport.cancelDownloads?.();
    this.store.clear();
  }
}
