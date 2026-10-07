// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { NoticeBanner, noticeBodyHtml, type AppNotice, type NoticesClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const notice = (overrides: Partial<AppNotice> = {}): AppNotice => ({
  id: "notice-1",
  title: "服务维护通知",
  body: "今晚 **22:00** 维护，详见 [公告](https://msime.app/news/)。",
  targets: [],
  channels: ["app"],
  published_at: "2026-10-01T08:00:00Z",
  ...overrides,
});

function client(items: AppNotice[]): NoticesClient {
  return {
    list: vi.fn().mockResolvedValue(items),
    dismiss: vi.fn().mockResolvedValue(undefined),
  };
}

test("renders each notice's Markdown and opens its links externally", async () => {
  const openExternalUrl = vi.fn();
  render(<NoticeBanner client={client([notice()])} openExternalUrl={openExternalUrl} />);
  const card = await screen.findByRole("article", { name: "服务维护通知" });
  expect(within(card).getByText("22:00").tagName).toBe("STRONG");

  fireEvent.click(within(card).getByRole("link", { name: "公告" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/news/");
});

test("a middle click on a link opens it externally instead of in the webview", async () => {
  const openExternalUrl = vi.fn();
  render(<NoticeBanner client={client([notice()])} openExternalUrl={openExternalUrl} />);
  const card = await screen.findByRole("article", { name: "服务维护通知" });
  const link = within(card).getByRole("link", { name: "公告" });
  const event = new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 });
  link.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
  expect(openExternalUrl).toHaveBeenCalledWith("https://msime.app/news/");
});

test("raw HTML, scripts and images in a body never become markup", () => {
  const html = noticeBodyHtml(
    '<img src=x onerror="alert(1)"> <script>alert(2)</script> ![追踪](https://example.com/p.png) [坏链接](javascript:alert(3))',
  );
  expect(html).not.toContain("<img");
  expect(html).not.toContain("<script");
  expect(html).not.toContain('href="javascript:');
  expect(html).toContain("&lt;script&gt;");
});

test("bare URLs become links", () => {
  expect(noticeBodyHtml("访问 https://msime.app 了解更多")).toContain(
    '<a href="https://msime.app">https://msime.app</a>',
  );
});

test("a link that is not HTTPS is not opened", async () => {
  const openExternalUrl = vi.fn();
  render(
    <NoticeBanner
      client={client([notice({ body: "[不安全](http://example.invalid/)" })])}
      openExternalUrl={openExternalUrl}
    />,
  );
  const card = await screen.findByRole("article", { name: "服务维护通知" });
  const link = card.querySelector("a");
  if (link) fireEvent.click(link);
  expect(openExternalUrl).not.toHaveBeenCalled();
});

test("closing a notice hides it and remembers the dismissal by id", async () => {
  const notices = client([notice(), notice({ id: "notice-2", title: "新版本发布" })]);
  render(<NoticeBanner client={notices} openExternalUrl={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "关闭公告 服务维护通知" }));

  expect(notices.dismiss).toHaveBeenCalledWith("notice-1");
  expect(screen.queryByRole("article", { name: "服务维护通知" })).toBeNull();
  expect(screen.getByRole("article", { name: "新版本发布" })).toBeDefined();
});

test("a failed fetch shows nothing", async () => {
  const notices: NoticesClient = {
    list: vi.fn().mockRejectedValue({ code: "notice_storage" }),
    dismiss: vi.fn(),
  };
  const { container } = render(<NoticeBanner client={notices} openExternalUrl={vi.fn()} />);
  await waitFor(() => expect(notices.list).toHaveBeenCalledTimes(1));
  expect(container.textContent).toBe("");
});
