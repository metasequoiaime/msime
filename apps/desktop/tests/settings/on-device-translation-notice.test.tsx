// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { OnDeviceTranslationNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("opens language settings and reports a failure", async () => {
  const openSettings = vi.fn().mockRejectedValue(new Error("unavailable"));
  const onError = vi.fn();
  render(
    <OnDeviceTranslationNotice
      languages={["英语", "日语"]}
      openSettings={openSettings}
      onError={onError}
    />,
  );

  expect(screen.getByText(/中文（简体）→ 英语、日语/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开语言与地区" }));

  await waitFor(() =>
    expect(onError).toHaveBeenCalledWith(
      "无法打开系统设置，请手动前往 系统设置 > 通用 > 语言与地区 > 翻译语言。",
    ),
  );
});

test("omits the settings button when the host cannot open settings", () => {
  render(<OnDeviceTranslationNotice languages={["英语"]} />);

  expect(screen.queryByRole("button", { name: "打开语言与地区" })).toBeNull();
});
