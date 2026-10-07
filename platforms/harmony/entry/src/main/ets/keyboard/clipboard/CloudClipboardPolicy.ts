/**
 * Decisions for the keyboard's 云端 clipboard section, kept free of ArkUI and HTTP so they can be checked off a device.
 *
 * The keyboard never uploads on copy and never reads the system clipboard for this: cloud items are fetched when the clipboard panel opens or the user asks for a refresh, and a local history entry reaches the cloud only through an explicit 发到云剪贴板. The text bounds are the shared ones from `client-core`'s `validate_clipboard_text`, measured in UTF-16 units as the service counts them.
 */
import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";

export interface CloudClipboardItem {
  readonly id: string;
  readonly text: string;
  readonly updatedAt: string;
}

export enum CloudClipboardState {
  SIGNED_OUT = "signed_out",
  DISABLED = "disabled",
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
  INVALID = "invalid",
  FAILED = "failed",
}

export const CLOUD_CLIPBOARD_TAB: string = "云端";
export const CLOUD_CLIPBOARD_SEND: string = "发到云剪贴板";
export const CLOUD_CLIPBOARD_SIGNED_OUT: string = "登录水杉账号后可在设备间同步剪贴板";
export const CLOUD_CLIPBOARD_DISABLED: string = "云剪贴板未开启";
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

interface Envelope {
  ok?: unknown;
  value?: unknown;
  error?: unknown;
}

interface RawPage {
  enabled?: unknown;
  items?: unknown;
}

interface RawItem {
  id?: unknown;
  text?: unknown;
  updated_at?: unknown;
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

  /**
   * Reads the account bridge's reply to `{operation:"clipboard", clipboard_operation:"list"}`.
   *
   * The page is checked whole, as `client-core`'s `validate_clipboard_page` does: a page with one malformed item is not shown at all, because dropping the bad rows would hide that the service sent something this client cannot honour. A disabled clipboard shows no items whatever the page carries.
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
    if (!page.enabled) return { state: CloudClipboardState.DISABLED, items: [] };
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
    if (envelope.error === "account_invalid") return CloudClipboardSendOutcome.INVALID;
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
    if (state === CloudClipboardState.FAILED) return CLOUD_CLIPBOARD_FAILED;
    return CloudClipboardPolicy.validText(text) ? null : CLOUD_CLIPBOARD_TOO_LONG;
  }

  /** The line the 云端 section shows above its items, or an empty string when the items speak for themselves. */
  static notice(state: CloudClipboardState | null, loading: boolean, count: number): string {
    if (loading) return CLOUD_CLIPBOARD_LOADING;
    if (state === null) return "";
    if (state === CloudClipboardState.SIGNED_OUT) return CLOUD_CLIPBOARD_SIGNED_OUT;
    if (state === CloudClipboardState.DISABLED) return CLOUD_CLIPBOARD_DISABLED;
    if (state === CloudClipboardState.FAILED) return CLOUD_CLIPBOARD_FAILED;
    return count === 0 ? CLOUD_CLIPBOARD_EMPTY : "";
  }

  /** What a finished send says, for the notice line. */
  static sendNotice(outcome: CloudClipboardSendOutcome): string {
    if (outcome === CloudClipboardSendOutcome.SENT) return CLOUD_CLIPBOARD_SENT;
    if (outcome === CloudClipboardSendOutcome.SIGNED_OUT) return CLOUD_CLIPBOARD_SIGNED_OUT;
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
      [...updatedAt].some((character: string): boolean => isControl(character.codePointAt(0) ?? 0))
    ) {
      return null;
    }
    return { id: raw.id, text: raw.text, updatedAt: updatedAt };
  }
}
