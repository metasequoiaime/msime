/**
 * Notices from the MSIME service, shown as a dismissible card above the settings page.
 *
 * client-core fetches the feed (`msime_client_notices`, at most once a minute), drops the ones the user dismissed and renders each Markdown body to HTML with raw HTML escaped and only http, https and mailto links kept. What is left for this host is reading that answer, wrapping the HTML in a page that can load nothing and run nothing, and deciding which link taps leave the application.
 */

/** One notice as the settings page shows it. */
export interface NoticeItem {
  id: string;
  title: string;
  /** The body rendered by client-core; safe to display, still never given scripts or network access. */
  html: string;
}

const MAX_TITLE_CHARACTERS: number = 200;

/** Light and dark text colours follow the system, matching the native card around the body. */
const PAGE_STYLE: string =
  'html,body{margin:0;padding:0;background:transparent;}' +
  'body{font:14px/1.5 sans-serif;color:#1f2329;word-wrap:break-word;overflow-wrap:anywhere;}' +
  'p{margin:0 0 6px;}ul,ol{margin:0 0 6px;padding-left:20px;}' +
  'a{color:#0a59f7;}code,pre{font-family:monospace;font-size:13px;}' +
  'pre{white-space:pre-wrap;margin:0 0 6px;}' +
  '@media (prefers-color-scheme: dark){body{color:#e5e5e5;}a{color:#5291ff;}}';

/**
 * Nothing may load and nothing may run: the body is already escaped, and this is the second wall. Inline styles are the page's own.
 */
const CONTENT_SECURITY_POLICY: string = "default-src 'none'; style-src 'unsafe-inline'";

function validText(value: unknown, maximum: number): value is string {
  return typeof value === 'string' && value.length > 0 && [...value].length <= maximum;
}

/** Keep the host-side id contract identical to client-core so dismissals can be persisted. */
function validId(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length <= 64 &&
    value.length > 0 &&
    [...value].every((character: string): boolean => /^[A-Za-z0-9_-]$/.test(character))
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
    if (parsed === null || typeof parsed !== 'object') return [];
    const envelope = parsed as Record<string, unknown>;
    if (envelope.ok !== true || envelope.value === null || typeof envelope.value !== 'object') return [];
    const items: unknown = (envelope.value as Record<string, unknown>).items;
    if (!Array.isArray(items)) return [];
    const notices: NoticeItem[] = [];
    for (const item of items as unknown[]) {
      if (item === null || typeof item !== 'object' || Array.isArray(item)) continue;
      const fields = item as Record<string, unknown>;
      if (!validId(fields.id) || !validText(fields.title, MAX_TITLE_CHARACTERS)) continue;
      if (typeof fields.html !== 'string') continue;
      notices.push({ id: fields.id, title: fields.title, html: fields.html });
    }
    return notices;
  }

  /** Whether a native reply (`{ok,value}` or `{ok:false,error}`) reports success. */
  static accepted(reply: string): boolean {
    try {
      const parsed: unknown = JSON.parse(reply);
      return parsed !== null && typeof parsed === 'object' && (parsed as Record<string, unknown>).ok === true;
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
   * The link to open in the system browser or mail application when the user taps it in a notice, or null for a navigation the card keeps (its own page) or refuses.
   *
   * Only the schemes client-core leaves in a body: anything else in a link was not written by the notice and is not followed.
   */
  static externalLink(url: string): string | null {
    const lower: string = url.toLowerCase();
    if (lower.startsWith('https://') || lower.startsWith('http://') || lower.startsWith('mailto:')) {
      return url;
    }
    return null;
  }

  /** Whether a navigation is the card loading its own page rather than a link being followed. */
  static ownPage(url: string): boolean {
    const lower: string = url.toLowerCase();
    return lower === '' || lower === 'about:blank' || lower.startsWith('data:');
  }
}
