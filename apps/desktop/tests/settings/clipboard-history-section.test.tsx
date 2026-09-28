// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ClipboardHistorySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("clipboard history loads entries and toggles a pin", async () => {
  const setPinned = vi.fn().mockResolvedValue(undefined);
  const clear = vi.fn().mockResolvedValue(undefined);
  const list = vi
    .fn()
    .mockResolvedValueOnce([{ text: "合成测试", timestampMs: 1_700_000_000_000, pinned: false }])
    .mockResolvedValueOnce([{ text: "合成测试", timestampMs: 1_700_000_000_000, pinned: true }]);

  render(
    <ClipboardHistorySection
      client={{ clear, list, setPinned }}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
    />,
  );

  expect(await screen.findByText("合成测试")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "固定剪贴板记录" }));

  await waitFor(() => expect(setPinned).toHaveBeenCalledWith("合成测试", true));
  expect(await screen.findByRole("button", { name: "取消固定剪贴板记录" })).toBeTruthy();
});
