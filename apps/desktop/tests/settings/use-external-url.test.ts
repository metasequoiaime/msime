// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { useExternalUrl } from "@msime/ui";

afterEach(() => {
  vi.restoreAllMocks();
});

test("browser fallback refuses non-HTTPS URLs before opening them", async () => {
  const open = vi.spyOn(window, "open").mockReturnValue({} as Window);
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ setError });

  await openExternalUrl("data:text/html,<script>alert(1)</script>");

  expect(open).not.toHaveBeenCalled();
  expect(setError).toHaveBeenCalledWith("无法打开外部链接，请稍后重试。");
});

test("browser fallback refuses HTTPS URLs with credentials or without a host", async () => {
  const open = vi.spyOn(window, "open").mockReturnValue({} as Window);
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ setError });

  await openExternalUrl("https://user:password@example.invalid/docs");
  await openExternalUrl("https:///docs");

  expect(open).not.toHaveBeenCalled();
  expect(setError).toHaveBeenCalledTimes(2);
});

test("browser fallback still opens HTTPS URLs", async () => {
  const open = vi.spyOn(window, "open").mockReturnValue({} as Window);
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ setError });

  await openExternalUrl("https://example.invalid/docs");

  expect(open).toHaveBeenCalledWith(
    "https://example.invalid/docs",
    "_blank",
    "noopener,noreferrer",
  );
  expect(setError).not.toHaveBeenCalled();
});
