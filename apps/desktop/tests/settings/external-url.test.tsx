// @vitest-environment jsdom
import { afterEach, describe, expect, test, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { isSafeExternalUrl, useExternalUrl } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Harness({ host }: { host?: (url: string) => Promise<void> }) {
  const open = useExternalUrl({
    openExternalUrl: host,
    setError: () => undefined,
  });
  return <button onClick={() => void open("https://example.test/docs")}>open</button>;
}

describe("external URL policy", () => {
  test.each([
    ["https://example.test/docs", true],
    ["https://example.test/docs?q=synthetic#part", true],
    ["http://example.test/docs", false],
    ["javascript:alert(1)", false],
    ["https://user:secret@example.test/docs", false],
    ["https://example.test/docs\nnext", false],
  ])("%s", (url, expected) => {
    expect(isSafeExternalUrl(url)).toBe(expected);
  });

  test("rejects an unsafe URL before invoking the host", async () => {
    const host = vi.fn(async () => undefined);
    const setError = vi.fn();
    function UnsafeHarness() {
      const open = useExternalUrl({ openExternalUrl: host, setError });
      return <button onClick={() => void open("http://example.test/docs")}>open</button>;
    }
    const view = render(<UnsafeHarness />);
    await act(async () => view.getByRole("button").click());
    expect(host).not.toHaveBeenCalled();
    expect(setError).toHaveBeenCalledWith("无法打开外部链接，请稍后重试。");
  });

  test("uses noopener browser fallback for a safe URL", async () => {
    const popup = { closed: false } as Window;
    const openWindow = vi.spyOn(window, "open").mockReturnValue(popup);
    const view = render(<Harness />);
    await act(async () => view.getByRole("button").click());
    expect(openWindow).toHaveBeenCalledWith("https://example.test/docs", "_blank", "noopener,noreferrer");
  });
});
