import MarkdownIt from "markdown-it";
import { useEffect, useState, type MouseEvent } from "react";
import { ActionButton } from "../core/action-button";

/** One live console notice, client-core's `notices::Notice`. */
export type AppNotice = {
  id: string;
  title: string;
  /** Simple Markdown. */
  body: string;
  targets: string[];
  channels: string[];
  /** RFC 3339. */
  published_at: string;
};

/** The host's notice feed. The host fetches it (the API refuses webview origins), caches it for the server's one minute and remembers dismissals per notice id on this device. */
export interface NoticesClient {
  /** The live notices the user has not dismissed, newest first. */
  list(): Promise<AppNotice[]>;
  dismiss(id: string): Promise<void>;
}

// Raw HTML in a body is shown as text, and images are not rendered so a notice never loads anything from the network; links are opened externally by the click handler below.
const markdown = new MarkdownIt({ html: false, linkify: true, breaks: true }).disable("image");

/** The HTML for a notice body; raw HTML in the source is escaped and images are left out. */
export function noticeBodyHtml(body: string): string {
  return markdown.render(body);
}

const externalLink = /^(https?:|mailto:)/i;

function publishedDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString("zh-CN");
}

/** The console's notices as dismissible cards at the top of the settings window. The feed is read once when the window opens; a failure shows nothing, since a notice is never needed to use the settings. */
export function NoticeBanner({
  client,
  openExternalUrl,
}: {
  client: NoticesClient;
  openExternalUrl: (url: string) => void | Promise<void>;
}) {
  const [notices, setNotices] = useState<AppNotice[]>([]);
  useEffect(() => {
    let active = true;
    client
      .list()
      .then((items) => {
        if (active) setNotices(items);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client]);
  if (notices.length === 0) return null;
  const dismiss = (id: string) => {
    setNotices((current) => current.filter((notice) => notice.id !== id));
    void client.dismiss(id).catch(() => undefined);
  };
  const openLink = (event: MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as Element).closest?.("a");
    if (!anchor) return;
    event.preventDefault();
    const href = anchor.getAttribute("href") ?? "";
    if (externalLink.test(href)) void openExternalUrl(href);
  };
  return (
    <section className="mb-3 flex flex-col gap-2" aria-label="公告">
      {notices.map((notice) => (
        <article
          key={notice.id}
          className="rounded-xl border border-[var(--divider-color)] bg-accent-soft px-3.5 py-3"
          aria-label={notice.title}
        >
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <strong className="text-body">{notice.title}</strong>
              {publishedDate(notice.published_at) && (
                <span className="ml-2 text-xs text-muted">
                  {publishedDate(notice.published_at)}
                </span>
              )}
            </div>
            <ActionButton
              action={() => dismiss(notice.id)}
              className="secondary shrink-0"
              ariaLabel={`关闭公告 ${notice.title}`}
              label="关闭"
            />
          </div>
          {notice.body && (
            <div
              className="mt-1.5 text-sm leading-relaxed text-secondary break-anywhere [&_a]:underline [&_p]:my-1 [&_ul]:my-1 [&_ul]:pl-5 [&_ol]:my-1 [&_ol]:pl-5"
              onClick={openLink}
              // A middle click would otherwise let the webview open the link in a window of its own (WebView2 does) instead of the system browser.
              onAuxClick={openLink}
              // markdown-it with html disabled escapes any HTML in the body, so this holds only the elements Markdown itself produces.
              dangerouslySetInnerHTML={{ __html: noticeBodyHtml(notice.body) }}
            />
          )}
        </article>
      ))}
    </section>
  );
}
