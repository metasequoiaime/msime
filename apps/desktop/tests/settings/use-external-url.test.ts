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

test("native host refuses unsafe URLs before receiving them", async () => {
  const hostOpenExternalUrl = vi.fn().mockResolvedValue(undefined);
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ openExternalUrl: hostOpenExternalUrl, setError });

  await openExternalUrl("https://user:password@example.invalid/docs");

  expect(hostOpenExternalUrl).not.toHaveBeenCalled();
  expect(setError).toHaveBeenCalledWith("无法打开外部链接，请稍后重试。");
});

test("浏览器成功隔离新标签页并返回 null 时不显示失败", async () => {
  const open = vi.spyOn(window, "open").mockReturnValue(null);
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

test("浏览器打开链接抛出异常时仍显示失败", async () => {
  vi.spyOn(window, "open").mockImplementation(() => {
    throw new Error("synthetic browser failure");
  });
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ setError });

  await openExternalUrl("https://example.invalid/docs");

  expect(setError).toHaveBeenCalledWith("无法打开外部链接，请稍后重试。");
});

test("原生宿主打开链接失败时仍显示失败", async () => {
  const hostOpenExternalUrl = vi.fn().mockRejectedValue(new Error("synthetic host failure"));
  const setError = vi.fn();
  const openExternalUrl = useExternalUrl({ openExternalUrl: hostOpenExternalUrl, setError });

  await openExternalUrl("https://example.invalid/docs");

  expect(hostOpenExternalUrl).toHaveBeenCalledWith("https://example.invalid/docs");
  expect(setError).toHaveBeenCalledWith("无法打开外部链接，请稍后重试。");
});
