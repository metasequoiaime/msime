// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HomePage, SettingsPage, type Snapshot } from "@msime/ui";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
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

const compositionEvents = [
  { name: "组词期间", event: { isComposing: true } },
  { name: "组词结束的兼容事件", event: { isComposing: false, keyCode: 229 } },
];

test.each(
  ["macos", "windows", "harmony"].flatMap((platform) =>
    ["Enter", "Escape"].flatMap((key) =>
      compositionEvents.map((event) => ({ platform, key, ...event })),
    ),
  ),
)("$platform 设置搜索保留$name 的 $key", async ({ platform, key, event }) => {
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockResolvedValue(undefined),
        host: testHost({ platform, mobile_settings: false }),
        windowControl: vi.fn(),
      }}
    />,
  );
  await settingsFormReady();
  const field = screen.getByRole("searchbox", { name: "搜索设置" }) as HTMLInputElement;
  fireEvent.change(field, { target: { value: "标点" } });

  expect(fireEvent.keyDown(field, { key, ...event })).toBe(true);
  expect(field.value).toBe("标点");
  expect(screen.queryByRole("heading", { name: /^(输入|输入设置)$/, level: 1 })).not.toBeNull();

  fireEvent.keyDown(field, { key, keyCode: key === "Enter" ? 13 : 27 });
  if (key === "Enter") {
    expect(
      screen.queryByRole("heading", { name: /^(标点与翻译|标点翻译)$/, level: 1 }),
    ).not.toBeNull();
  } else {
    expect(field.value).toBe("");
    expect(screen.queryByRole("heading", { name: /^(输入|输入设置)$/, level: 1 })).not.toBeNull();
  }
});

test.each(compositionEvents)("触屏首页搜索保留$name 的 Escape", ({ event }) => {
  const onOpenPage = vi.fn();
  render(
    <ToastProvider>
      <HomePage
        preferences={initial.preferences}
        onOpenPage={onOpenPage}
        touchLayout
        rootPages={[{ id: "expression", title: "表达" }]}
      />
    </ToastProvider>,
  );
  const field = screen.getByRole("searchbox", { name: "搜索设置" }) as HTMLInputElement;
  fireEvent.change(field, { target: { value: "表达" } });

  expect(fireEvent.keyDown(field, { key: "Escape", ...event })).toBe(true);
  expect(field.value).toBe("表达");
  expect(onOpenPage).not.toHaveBeenCalled();

  expect(fireEvent.keyDown(field, { key: "Escape", keyCode: 27 })).toBe(false);
  expect(field.value).toBe("");
});
