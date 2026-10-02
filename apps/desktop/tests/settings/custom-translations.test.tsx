// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import {
  SETTINGS_AUTOSAVE_DELAY_MS,
  SettingsPage,
  parseCustomTranslations,
  useCustomTranslations,
  type CustomTranslationsClient,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

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
        host: testHost({ platform: "harmony" }),
        customTranslations: { load: async () => "你好\thello\n", save },
      }}
    />,
  );
  const field = (await screen.findByLabelText("自定义候选释义")) as HTMLTextAreaElement;
  expect(field.value).toBe("你好\thello\n");
  fireEvent.change(field, { target: { value: "刚才\tjust now\n" } });
  expect(screen.queryByRole("button", { name: "保存自定义释义" })).toBeNull();
  // Saved on its own once typing pauses.
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith("刚才\tjust now\n");
  expect(screen.getByText("1 条释义")).toBeTruthy();
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
        host: testHost({ platform: "macos" }),
        customTranslations: { load: async () => "", save },
      }}
    />,
  );
  const field = (await screen.findByLabelText("自定义候选释义")) as HTMLTextAreaElement;
  expect(field.value).toBe("");
  fireEvent.change(field, { target: { value: "你好\thello\n" } });
  // Leaving the field saves at once rather than waiting out the countdown.
  fireEvent.blur(field);
  await waitFor(() => expect(save).toHaveBeenCalledWith("你好\thello\n"));
  await screen.findByText("已保存");
});

test("a host without the route is not offered the section", async () => {
  render(
    <SettingsPage
      initialPage="expression"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByLabelText("自定义候选释义")).toBeNull();
});

function overlayClient(save: CustomTranslationsClient["save"]): CustomTranslationsClient {
  return { load: async () => "", save };
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

test("typing saves once, after the edits pause", async () => {
  vi.useFakeTimers();
  const save = vi.fn().mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("你"));
  await advance(SETTINGS_AUTOSAVE_DELAY_MS - 100);
  act(() => result.current.setText("你好\thello"));
  await advance(SETTINGS_AUTOSAVE_DELAY_MS - 100);
  expect(save).not.toHaveBeenCalled();

  await advance(100);
  expect(save).toHaveBeenCalledExactlyOnceWith("你好\thello");
  expect(result.current.saveState).toBe("saved");
});

test("a failed save keeps the edit and 重试 writes it again", async () => {
  vi.useFakeTimers();
  const save = vi.fn().mockRejectedValueOnce(new Error("磁盘已满")).mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("你好\thello"));
  await advance(SETTINGS_AUTOSAVE_DELAY_MS);
  expect(result.current.saveState).toBe("failed");
  expect(result.current.saveError).toBe("磁盘已满");

  await act(() => result.current.flush());
  expect(save).toHaveBeenCalledTimes(2);
  expect(save).toHaveBeenLastCalledWith("你好\thello");
  expect(result.current.saveState).toBe("saved");
});

test("an edit made while a save is in flight is saved after it", async () => {
  vi.useFakeTimers();
  let finish: () => void = () => undefined;
  const save = vi
    .fn()
    .mockImplementationOnce(() => new Promise<void>((resolve) => (finish = resolve)))
    .mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("a\tb"));
  await advance(SETTINGS_AUTOSAVE_DELAY_MS);
  expect(result.current.saveState).toBe("saving");
  act(() => result.current.setText("a\tc"));
  await act(async () => finish());

  expect(save).toHaveBeenCalledTimes(2);
  expect(save).toHaveBeenLastCalledWith("a\tc");
  expect(result.current.saveState).toBe("saved");
});

test("leaving the page saves the pending edit at once", async () => {
  vi.useFakeTimers();
  const save = vi.fn().mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("你好\thello"));
  await act(async () => {
    window.dispatchEvent(new Event("pagehide"));
  });
  expect(save).toHaveBeenCalledExactlyOnceWith("你好\thello");
});

test("closing the page with an edit still counting down writes it", async () => {
  vi.useFakeTimers();
  const save = vi.fn().mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result, unmount } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("你好\thello"));
  unmount();
  await advance(0);
  expect(save).toHaveBeenCalledExactlyOnceWith("你好\thello");
});

test("an oversized overlay is not written", async () => {
  vi.useFakeTimers();
  const save = vi.fn().mockResolvedValue(undefined);
  const client = overlayClient(save);
  const { result } = renderHook(() => useCustomTranslations({ client }));
  await advance(0);

  act(() => result.current.setText("x".repeat(1024 * 1024 + 1)));
  await advance(SETTINGS_AUTOSAVE_DELAY_MS);
  expect(save).not.toHaveBeenCalled();
  expect(result.current.notice).toBe("自定义释义过大，请精简后再保存。");
});
