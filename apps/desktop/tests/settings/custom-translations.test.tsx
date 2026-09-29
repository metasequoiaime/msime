// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  SettingsPage,
  parseCustomTranslations,
  type HostCapabilities,
  type Snapshot,
} from "@msime/ui";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

// Every rule here is EnglishDictionary::load_custom_translations. A line this accepts and the
// Engine drops is a line the page promised to apply and did not.
test("the overlay is read the way the Engine reads it", () => {
  const report = parseCustomTranslations(
    "﻿# a comment\n\n你好\thello\r\nserendipity\t意外发现珍奇事物的本领\n  刚才  \t  just now  \n",
  );
  expect(report.entries).toHaveLength(3);
  expect(report.entries[0]).toEqual({ source: "你好", gloss: "hello", chinese: true });
  expect(report.entries[1].chinese).toBe(false);
  // Surrounding whitespace on both halves is trimmed, as the Engine trims it.
  expect(report.entries[2]).toEqual({ source: "刚才", gloss: "just now", chinese: true });
  expect(report.skipped).toBe(0);
});

test("a line the Engine would drop is counted rather than silently kept", () => {
  const report = parseCustomTranslations("nogloss\t\n\tnosource\nnotabhere\n你好\thello\n");
  expect(report.entries).toHaveLength(1);
  // "notabhere" carries no tab at all, which the Engine skips like the two malformed ones.
  expect(report.skipped).toBe(3);
});

test("the last spelling of a source wins, as the Engine's map assignment does", () => {
  const report = parseCustomTranslations("你好\thi\n你好\thello\n");
  expect(report.entries).toEqual([{ source: "你好", gloss: "hello", chinese: true }]);
});

test("a host with nowhere to drop a file can still supply the overlay", async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "harmony" } as HostCapabilities,
        customTranslations: { load: async () => "你好\thello\n", save },
      }}
    />,
  );
  const field = (await screen.findByLabelText("自定义候选释义")) as HTMLTextAreaElement;
  expect(field.value).toBe("你好\thello\n");
  fireEvent.change(field, { target: { value: "刚才\tjust now\n" } });
  fireEvent.click(screen.getByRole("button", { name: "保存自定义释义" }));
  await screen.findByText(/已保存 1 条释义/);
  expect(save).toHaveBeenCalledWith("刚才\tjust now\n");
});

test("the desktop hosts get the overlay too, where the profile directory is hidden", async () => {
  // The reference tells the user to drop the file into the profile directory. On macOS that path is
  // inside ~/Library, which the Finder hides, so the instruction does not carry over and the page is
  // the way in. This pins that the section is offered to a desktop host, not only to a sandboxed one.
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "macos" } as HostCapabilities,
        customTranslations: { load: async () => "", save },
      }}
    />,
  );
  const field = (await screen.findByLabelText("自定义候选释义")) as HTMLTextAreaElement;
  expect(field.value).toBe("");
  fireEvent.change(field, { target: { value: "你好\thello\n" } });
  fireEvent.click(screen.getByRole("button", { name: "保存自定义释义" }));
  await screen.findByText(/已保存 1 条释义/);
  expect(save).toHaveBeenCalledWith("你好\thello\n");
});

test("a host without the route is not offered the section", async () => {
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: { platform: "windows" } as HostCapabilities,
      }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  expect(screen.queryByLabelText("自定义候选释义")).toBeNull();
});
