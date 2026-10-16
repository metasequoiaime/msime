import { TextPolicy } from "../keyboard/TextPolicy";
/**
 * 来自水杉服务的公告，以可关闭的卡片形式显示在设置页上方。
 *
 * client-services 负责拉取公告列表（`msime_client_notices`，至多每分钟一次），去掉用户已关闭的公告，并把每条 Markdown 正文渲染成 HTML：原始 HTML 一律转义，只保留 http、https 和 mailto 链接。留给本宿主的只有读取这份应答、把 HTML 包进一个什么都不能加载也什么都不能执行的页面，以及决定点哪些链接会离开应用。
 */

/** One notice as the settings page shows it. */
export interface NoticeItem {
  id: string;
  title: string;
  /** 由 client-services 渲染的正文；可以安全显示，但仍然从不给它脚本或网络权限。 */
  html: string;
}

const MAX_TITLE_CHARACTERS: number = 200;

/** Light and dark text colours follow the system, matching the native card around the body. */
const PAGE_STYLE: string =
  "html,body{margin:0;padding:0;background:transparent;}" +
  "body{font:14px/1.5 sans-serif;color:#1f2329;word-wrap:break-word;overflow-wrap:anywhere;}" +
  "p{margin:0 0 6px;}ul,ol{margin:0 0 6px;padding-left:20px;}" +
  "a{color:#0a59f7;}code,pre{font-family:monospace;font-size:13px;}" +
  "pre{white-space:pre-wrap;margin:0 0 6px;}" +
  "@media (prefers-color-scheme: dark){body{color:#e5e5e5;}a{color:#5291ff;}}";

/**
 * Nothing may load and nothing may run: the body is already escaped, and this is the second wall. Inline styles are the page's own.
 */
const CONTENT_SECURITY_POLICY: string = "default-src 'none'; style-src 'unsafe-inline'";

function validText(value: unknown, maximum: number): value is string {
  return (
    typeof value === "string" && value.length > 0 && TextPolicy.characters(value).length <= maximum
  );
}

/** 宿主侧的 id 约定与 client-services 保持一致，这样关闭记录才能持久化。 */
function validId(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length <= 64 &&
    value.length > 0 &&
    TextPolicy.characters(value).every((character: string): boolean =>
      /^[A-Za-z0-9_-]$/.test(character),
    )
  );
}

export class NoticePolicy {
  /** Whether a notice response still belongs to the newest page request. */
  static requestCurrent(expectedGeneration: number, actualGeneration: number): boolean {
    return (
      Number.isSafeInteger(expectedGeneration) &&
      Number.isSafeInteger(actualGeneration) &&
      expectedGeneration >= 0 &&
      expectedGeneration === actualGeneration
    );
  }

  /** The notices in a `notices` reply, newest first, or an empty list when the reply is a refusal or malformed. */
  static items(reply: string): NoticeItem[] {
    let parsed: unknown;
    try {
      parsed = JSON.parse(reply);
    } catch {
      return [];
    }
    if (parsed === null || typeof parsed !== "object") return [];
    const envelope = parsed as Record<string, unknown>;
    if (envelope.ok !== true || envelope.value === null || typeof envelope.value !== "object")
      return [];
    const items: unknown = (envelope.value as Record<string, unknown>).items;
    if (!Array.isArray(items)) return [];
    const notices: NoticeItem[] = [];
    for (const item of items as unknown[]) {
      if (item === null || typeof item !== "object" || Array.isArray(item)) continue;
      const fields = item as Record<string, unknown>;
      if (!validId(fields.id) || !validText(fields.title, MAX_TITLE_CHARACTERS)) continue;
      if (typeof fields.html !== "string") continue;
      notices.push({ id: fields.id, title: fields.title, html: fields.html });
    }
    return notices;
  }

  /** Whether a native reply (`{ok,value}` or `{ok:false,error}`) reports success. */
  static accepted(reply: string): boolean {
    try {
      const parsed: unknown = JSON.parse(reply);
      return (
        parsed !== null &&
        typeof parsed === "object" &&
        (parsed as Record<string, unknown>).ok === true
      );
    } catch {
      return false;
    }
  }

  /** The notices left once `id` is dismissed. */
  static without(items: NoticeItem[], id: string): NoticeItem[] {
    return items.filter((item: NoticeItem): boolean => item.id !== id);
  }

  /** A complete document for the body: no script, no network, the system's colours. */
  static page(html: string): string {
    return (
      '<!DOCTYPE html><html><head><meta charset="utf-8">' +
      `<meta http-equiv="Content-Security-Policy" content="${CONTENT_SECURITY_POLICY}">` +
      '<meta name="viewport" content="width=device-width,initial-scale=1">' +
      `<style>${PAGE_STYLE}</style></head><body>${html}</body></html>`
    );
  }

  /**
   * 用户在公告里点击链接时要交给系统浏览器或邮件应用打开的地址；对于卡片自己保留（它自己的页面）或拒绝的跳转，返回 null。
   *
   * 只认 client-services 在正文里保留的那几种 scheme：链接里出现的其他 scheme 都不是公告作者写的，不予跟随。
   */
  static externalLink(url: string): string | null {
    const lower: string = url.toLowerCase();
    if (
      lower.startsWith("https://") ||
      lower.startsWith("http://") ||
      lower.startsWith("mailto:")
    ) {
      return url;
    }
    return null;
  }

  /** Whether a navigation is the card loading its own page rather than a link being followed. */
  static ownPage(url: string): boolean {
    const lower: string = url.toLowerCase();
    return lower === "" || lower === "about:blank" || lower.startsWith("data:");
  }
}
