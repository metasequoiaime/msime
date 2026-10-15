import MarkdownIt from "markdown-it";
import type { MouseEvent } from "react";

// 原始 HTML 按文字显示，图片不渲染；生成的链接由调用处交给宿主打开。
const markdown = new MarkdownIt({ html: false, linkify: true, breaks: true }).disable("image");

export function linkedMarkdownHtml(body: string): string {
  return markdown.render(body);
}

/** 普通点击与辅助点击共用链接委托，允许打开的协议由调用处决定。 */
export function createMarkdownLinkHandler(
  openUrl: ((url: string) => void | Promise<void>) | undefined,
  allowedLink: RegExp,
) {
  return (event: MouseEvent<HTMLElement>) => {
    const anchor = (event.target as Element).closest?.("a");
    if (!anchor) return;
    event.preventDefault();
    const href = anchor.getAttribute("href") ?? "";
    if (allowedLink.test(href)) void openUrl?.(href);
  };
}
