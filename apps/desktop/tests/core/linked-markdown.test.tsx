// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  createMarkdownLinkHandler,
  linkedMarkdownHtml,
} from "../../../../packages/ui/src/core/linked-markdown";

afterEach(cleanup);

test("renders linked Markdown formatting without raw HTML or images", () => {
  const html = linkedMarkdownHtml(
    '**强调**\n下一行\n\n- 列表\n\nhttps://example.invalid\n\n<img src=x onerror="alert(1)"><script>alert(2)</script>\n\n![图片](https://example.invalid/p.png) [坏链接](javascript:alert(3))',
  );
  expect(html).toContain("<strong>强调</strong><br>\n下一行");
  expect(html).toContain("<li>列表</li>");
  expect(html).toContain('<a href="https://example.invalid">');
  expect(html).toContain("&lt;script&gt;");
  expect(html).not.toMatch(/<img|<script|href="javascript:/);
});

test.each(["click", "auxclick"])("delegates a nested link %s and cancels navigation", (type) => {
  const open = vi.fn();
  const handler = createMarkdownLinkHandler(open, /^https:/i);
  render(
    <div onClick={handler} onAuxClick={handler}>
      <a href="HTTPS://example.invalid/path">
        <strong>嵌套链接</strong>
      </a>
    </div>,
  );
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: type === "auxclick" ? 1 : 0,
  });
  screen.getByText("嵌套链接").dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
  expect(open).toHaveBeenCalledExactlyOnceWith("HTTPS://example.invalid/path");
});

test.each(["http://example.invalid", "mailto:synthetic@example.invalid", "#section", ""])(
  "cancels a disallowed link %s without calling the host",
  (href) => {
    const open = vi.fn();
    render(
      <div onClick={createMarkdownLinkHandler(open, /^https:/i)}>
        <a href={href}>链接</a>
      </div>,
    );
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    screen.getByText("链接").dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(open).not.toHaveBeenCalled();
  },
);

test("still cancels navigation when the host callback is absent", () => {
  render(
    <div onClick={createMarkdownLinkHandler(undefined, /^https:/i)}>
      <a href="https://example.invalid">链接</a>
    </div>,
  );
  const event = new MouseEvent("click", { bubbles: true, cancelable: true });
  screen.getByText("链接").dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
});

test("ignores ordinary text and cancels anchors without href", () => {
  const open = vi.fn();
  render(
    <div onClick={createMarkdownLinkHandler(open, /^https:/i)}>
      <span>普通文字</span>
      <a>无地址</a>
    </div>,
  );
  const plain = new MouseEvent("click", { bubbles: true, cancelable: true });
  screen.getByText("普通文字").dispatchEvent(plain);
  expect(plain.defaultPrevented).toBe(false);
  const anchor = new MouseEvent("click", { bubbles: true, cancelable: true });
  screen.getByText("无地址").dispatchEvent(anchor);
  expect(anchor.defaultPrevented).toBe(true);
  expect(open).not.toHaveBeenCalled();
});
