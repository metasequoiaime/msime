/**
 * 键盘云端剪贴板区的各项决策，不依赖 ArkUI 和 HTTP，因此可以脱离设备检查。
 *
 * 键盘从不在复制时上传，也从不为此读取系统剪贴板：云端条目在剪贴板面板打开或用户要求刷新时获取，本地历史条目只有经用户明确执行「发到云剪贴板」才会到达云端。文本上限是 `client-services` 的 `validate_clipboard_text` 中共用的那组，和服务一样按 UTF-16 单元计量。
 */
import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";

export interface CloudClipboardItem {
  readonly id: string;
  readonly text: string;
  readonly updatedAt: string;
  readonly pinned: boolean;
  /** 写入这条的设备名，可以为空。 */
  readonly device: string;
}

export enum CloudClipboardState {
  SIGNED_OUT = "signed_out",
  DISABLED = "disabled",
  /** 本机还没被账号下已有的设备批准：读不到条目，也不上传。 */
  PENDING = "pending",
  READY = "ready",
  FAILED = "failed",
}

export interface CloudClipboardListing {
  readonly state: CloudClipboardState;
  readonly items: CloudClipboardItem[];
}

export enum CloudClipboardSendOutcome {
  SENT = "sent",
  SIGNED_OUT = "signed_out",
  PENDING = "pending",
  INVALID = "invalid",
  FAILED = "failed",
}

export const CLOUD_CLIPBOARD_TAB: string = "云端";
export const CLOUD_CLIPBOARD_SEND: string = "发到云剪贴板";
export const CLOUD_CLIPBOARD_SIGNED_OUT: string = "登录水杉账号后可在设备间同步剪贴板";
export const CLOUD_CLIPBOARD_DISABLED: string = "云剪贴板未开启";
export const CLOUD_CLIPBOARD_PENDING: string = "本机等待批准：请在已开启云剪贴板的设备上点「允许」";
export const CLOUD_CLIPBOARD_LOADING: string = "正在读取云剪贴板…";
export const CLOUD_CLIPBOARD_FAILED: string = "云剪贴板暂时无法读取，请稍后刷新";
export const CLOUD_CLIPBOARD_EMPTY: string = "云剪贴板里还没有内容";
export const CLOUD_CLIPBOARD_SENT: string = "已发到云剪贴板";
export const CLOUD_CLIPBOARD_SEND_FAILED: string = "发送失败，请稍后重试";
export const CLOUD_CLIPBOARD_TOO_LONG: string =
  "这条内容超过 4,000 字或含有无法同步的字符，不能发到云剪贴板";

const MAX_ITEMS: number = 50;
const MAX_TEXT_UNITS: number = 4000;
const MAX_UPDATED_AT_BYTES: number = 128;
/** 驱动交回的设备名已去掉控制字符并截到 64 个字符。 */
const MAX_DEVICE_CHARS: number = 64;

interface Envelope {
  ok?: unknown;
  value?: unknown;
  error?: unknown;
}

interface RawPage {
  enabled?: unknown;
  items?: unknown;
  e2e?: unknown;
}

interface RawE2e {
  status?: unknown;
}

interface RawItem {
  id?: unknown;
  text?: unknown;
  updated_at?: unknown;
  pinned?: unknown;
  device?: unknown;
}

/** A Unicode control character (`Cc`): U+0000–U+001F and U+007F–U+009F, which is what Rust's `char::is_control` tests. */
function isControl(code: number): boolean {
  return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
}

function parseObject(document: string): Envelope | null {
  try {
    const value: unknown = JSON.parse(document);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Envelope)
      : null;
  } catch {
    return null;
  }
}

export class CloudClipboardPolicy {
  static readonly MAX_ITEMS: number = MAX_ITEMS;
  static readonly MAX_TEXT_UNITS: number = MAX_TEXT_UNITS;

  /**
   * Whether text may be stored in the cloud clipboard.
   *
   * Line breaks and tabs are content in a clipboard, so they pass; every other control character is refused, as is NUL and anything blank. The same rule decides what the keyboard will offer to send and what the account bridge will forward, so neither can disagree with the service on its own.
   */
  static validText(text: unknown): text is string {
    if (typeof text !== "string" || text.trim().length === 0 || text.length > MAX_TEXT_UNITS) {
      return false;
    }
    for (let index: number = 0; index < text.length; index++) {
      const code: number = text.charCodeAt(index);
      if (isControl(code) && code !== 0x09 && code !== 0x0a && code !== 0x0d) {
        return false;
      }
    }
    return TextPolicy.validUnicode(text);
  }

  /**
   * Whether the focused editor may see cloud items at all.
   *
   * A password field never does, and neither does an editor whose attributes have not arrived yet: until they do, the keyboard's traits still describe the previous field, which is the same reason key sounds and statistics wait for them.
   */
  static editorAllows(attributesKnown: boolean, password: boolean): boolean {
    return attributesKnown && !password;
  }

  /** Whether a listing fetched for one editor still belongs to the focused one. */
  static current(expectedEditor: number, actualEditor: number): boolean {
    return (
      Number.isSafeInteger(expectedEditor) &&
      Number.isSafeInteger(actualEditor) &&
      expectedEditor >= 0 &&
      expectedEditor === actualEditor
    );
  }

  /** 云端条目只有在当前编辑框仍可见且账号会话仍与读取结果一致时才能使用。 */
  static canUseRows(
    visible: boolean,
    state: CloudClipboardState | null,
    expectedBinding: string,
    currentBinding: string,
  ): boolean {
    return (
      visible &&
      state === CloudClipboardState.READY &&
      expectedBinding.length > 0 &&
      expectedBinding === currentBinding
    );
  }

  /** Whether a completed send still belongs to the currently active send request. */
  static sendCurrent(expectedGeneration: number, actualGeneration: number): boolean {
    return (
      Number.isSafeInteger(expectedGeneration) &&
      Number.isSafeInteger(actualGeneration) &&
      expectedGeneration >= 0 &&
      expectedGeneration === actualGeneration
    );
  }

  /**
   * 读取账号桥对 `{operation:"clipboard", clipboard_operation:"list"}` 的应答。
   *
   * 整页一起检查，与 `client-services` 的 `validate_clipboard_page` 一样：只要有一个条目格式不对，整页都不显示，因为丢掉坏行会掩盖服务发来了本客户端无法遵从的内容。剪贴板被停用时，不论页面带了什么都不显示条目。
   *
   * 应答是端到端加密驱动 `list` 的结果：`{enabled, retention_days, items, e2e:{status, code, pending_devices, undecryptable}}`。`e2e.status` 为 `pending` 时本机还在等待批准，读作 `PENDING`，不显示条目；解不开而被驱动略去的条目不算错误。
   */
  static parseList(reply: string): CloudClipboardListing {
    const failed: CloudClipboardListing = { state: CloudClipboardState.FAILED, items: [] };
    const envelope: Envelope | null = parseObject(reply);
    if (envelope === null) return failed;
    if (envelope.ok !== true) {
      return envelope.error === "account_unauthorized"
        ? { state: CloudClipboardState.SIGNED_OUT, items: [] }
        : failed;
    }
    const value: unknown = envelope.value;
    if (value === null || typeof value !== "object" || Array.isArray(value)) return failed;
    const page: RawPage = value as RawPage;
    if (typeof page.enabled !== "boolean" || !Array.isArray(page.items)) return failed;
    const e2e: unknown = page.e2e;
    if (e2e === null || typeof e2e !== "object" || Array.isArray(e2e)) return failed;
    const status: unknown = (e2e as RawE2e).status;
    if (status !== "ready" && status !== "pending" && status !== "disabled") return failed;
    if (!page.enabled || status === "disabled")
      return { state: CloudClipboardState.DISABLED, items: [] };
    if (status === "pending") return { state: CloudClipboardState.PENDING, items: [] };
    const rows: unknown[] = page.items as unknown[];
    if (rows.length > MAX_ITEMS) return failed;
    const items: CloudClipboardItem[] = [];
    for (const row of rows) {
      const item: CloudClipboardItem | null = CloudClipboardPolicy.item(row);
      if (item === null) return failed;
      items.push(item);
    }
    return { state: CloudClipboardState.READY, items: items };
  }

  /** Reads the bridge's reply to an `add`. An expired session is the one failure worth naming, since signing in again is something the user can do. */
  static parseSend(reply: string): CloudClipboardSendOutcome {
    const envelope: Envelope | null = parseObject(reply);
    if (envelope === null) return CloudClipboardSendOutcome.FAILED;
    if (envelope.ok === true) return CloudClipboardSendOutcome.SENT;
    if (envelope.error === "account_unauthorized") return CloudClipboardSendOutcome.SIGNED_OUT;
    if (envelope.error === "clipboard_device_pending") return CloudClipboardSendOutcome.PENDING;
    if (envelope.error === "account_invalid" || envelope.error === "invalid_clipboard_text")
      return CloudClipboardSendOutcome.INVALID;
    return CloudClipboardSendOutcome.FAILED;
  }

  /**
   * Why 发到云剪贴板 is unavailable for this text, or null when it may be sent.
   *
   * Sending needs a listing that said the account is signed in and its clipboard is on. Before one has arrived the answer is not known, and offering the action on a guess would turn a disabled clipboard into a server refusal that looks like an expired session.
   */
  static sendBlock(state: CloudClipboardState | null, text: string): string | null {
    if (state === null) return CLOUD_CLIPBOARD_LOADING;
    if (state === CloudClipboardState.SIGNED_OUT) return CLOUD_CLIPBOARD_SIGNED_OUT;
    if (state === CloudClipboardState.DISABLED) return CLOUD_CLIPBOARD_DISABLED;
    if (state === CloudClipboardState.PENDING) return CLOUD_CLIPBOARD_PENDING;
    if (state === CloudClipboardState.FAILED) return CLOUD_CLIPBOARD_FAILED;
    return CloudClipboardPolicy.validText(text) ? null : CLOUD_CLIPBOARD_TOO_LONG;
  }

  /** The line the 云端 section shows above its items, or an empty string when the items speak for themselves. */
  static notice(state: CloudClipboardState | null, loading: boolean, count: number): string {
    if (loading) return CLOUD_CLIPBOARD_LOADING;
    if (state === null) return "";
    if (state === CloudClipboardState.SIGNED_OUT) return CLOUD_CLIPBOARD_SIGNED_OUT;
    if (state === CloudClipboardState.DISABLED) return CLOUD_CLIPBOARD_DISABLED;
    if (state === CloudClipboardState.PENDING) return CLOUD_CLIPBOARD_PENDING;
    if (state === CloudClipboardState.FAILED) return CLOUD_CLIPBOARD_FAILED;
    return count === 0 ? CLOUD_CLIPBOARD_EMPTY : "";
  }

  /** What a finished send says, for the notice line. */
  static sendNotice(outcome: CloudClipboardSendOutcome): string {
    if (outcome === CloudClipboardSendOutcome.SENT) return CLOUD_CLIPBOARD_SENT;
    if (outcome === CloudClipboardSendOutcome.SIGNED_OUT) return CLOUD_CLIPBOARD_SIGNED_OUT;
    if (outcome === CloudClipboardSendOutcome.PENDING) return CLOUD_CLIPBOARD_PENDING;
    if (outcome === CloudClipboardSendOutcome.INVALID) return CLOUD_CLIPBOARD_TOO_LONG;
    return CLOUD_CLIPBOARD_SEND_FAILED;
  }

  private static item(row: unknown): CloudClipboardItem | null {
    if (row === null || typeof row !== "object" || Array.isArray(row)) return null;
    const raw: RawItem = row as RawItem;
    if (typeof raw.id !== "string" || !/^[0-9a-f]{64}$/.test(raw.id)) return null;
    if (!CloudClipboardPolicy.validText(raw.text)) return null;
    const updatedAt: unknown = raw.updated_at;
    if (
      typeof updatedAt !== "string" ||
      updatedAt.length === 0 ||
      utf8Length(updatedAt) > MAX_UPDATED_AT_BYTES ||
      TextPolicy.characters(updatedAt).some((character: string): boolean =>
        isControl(character.codePointAt(0) ?? 0),
      )
    ) {
      return null;
    }
    const device: unknown = raw.device;
    const deviceCharacters: string[] =
      typeof device === "string" ? TextPolicy.characters(device) : [];
    if (
      typeof raw.pinned !== "boolean" ||
      typeof device !== "string" ||
      deviceCharacters.length > MAX_DEVICE_CHARS ||
      deviceCharacters.some((character: string): boolean =>
        isControl(character.codePointAt(0) ?? 0),
      )
    ) {
      return null;
    }
    return { id: raw.id, text: raw.text, updatedAt: updatedAt, pinned: raw.pinned, device: device };
  }
}
