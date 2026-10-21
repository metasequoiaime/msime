// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

afterEach(cleanup);

async function submitReport(platform: "macos" | "linux", detail: string) {
  const openExternalUrl = vi.fn<(url: string) => Promise<void>>().mockResolvedValue(undefined);
  const copyText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="feedback"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        host: testHost({ platform }),
        openExternalUrl,
        copyText,
      }}
    />,
  );
  const description = await screen.findByRole("textbox", { name: "反馈描述" });
  await waitFor(() => expect((description as HTMLTextAreaElement).disabled).toBe(false));
  fireEvent.change(description, { target: { value: detail } });
  fireEvent.click(screen.getByRole("button", { name: "复制报告" }));
  await waitFor(() => expect(copyText).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("button", { name: "在 GitHub 提交" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledOnce());
  return { url: openExternalUrl.mock.calls[0][0], report: copyText.mock.calls[0][0] };
}

test.each([
  ["macos", "合成描述".repeat(150)],
  ["linux", "合成描述".repeat(150)],
  ["macos", "&+=%".repeat(1000)],
  ["linux", "a😀".repeat(1000)],
] as const)("%s 的长反馈按完整网址预算保留可预填的最长前缀", async (platform, detail) => {
  const { url, report } = await submitReport(platform, detail);

  // 宿主在启动浏览器前拒绝超过 4096 字节的网址；表单编码后的网址是 ASCII。
  expect(url).toMatch(/^[\x00-\x7f]+$/);
  expect(url.length).toBeLessThanOrEqual(4096);
  const parsed = new URL(url);
  expect(parsed.origin).toBe("https://github.com");
  expect(parsed.pathname).toBe("/metasequoiaime/msime/issues/new");
  expect(parsed.searchParams.get("title")).toBe("功能异常");
  const body = parsed.searchParams.get("body")!;
  expect(body).toContain("### 描述\n");
  expect(body).not.toContain("\ufffd");
  expect(report.startsWith(body)).toBe(true);
  expect(body.length).toBeLessThan(report.length);
  expect(report).toContain(detail);
  expect(report).toContain("### 环境\n");

  const nextCharacter = Array.from(report)[Array.from(body).length];
  parsed.searchParams.set("body", body + nextCharacter);
  expect(parsed.href.length).toBeGreaterThan(4096);
});

test("短反馈的预填正文与完整复制报告一致", async () => {
  const { url, report } = await submitReport("macos", "合成短描述 😀 & +");

  expect(new URL(url).searchParams.get("body")).toBe(report);
  expect(report).toContain("合成短描述 😀 & +");
  expect(report).toContain("### 环境\n");
});
