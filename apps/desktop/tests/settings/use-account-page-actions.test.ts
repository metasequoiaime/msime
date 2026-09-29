// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useAccountPageActions } from "@msime/ui";

test("builds mobile account actions and reports cloud action failures", async () => {
  const openCloudDictionary = vi.fn().mockRejectedValue(new Error("synthetic"));
  const openCloudClipboard = vi.fn().mockResolvedValue(undefined);
  const setError = vi.fn();
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  const finishAccountLogin = vi.fn();
  const openAbout = vi.fn();
  const openCommunity = vi.fn();
  const { result } = renderHook(() =>
    useAccountPageActions({
      hasAccountLoginReturnPage: true,
      finishAccountLogin,
      openLocalDesigns: vi.fn(),
      openCommunity,
      openCloudDictionary,
      openCloudClipboard,
      mobile: true,
      canOpenExternalUrl: true,
      openExternalUrl,
      onReplayOnboarding: vi.fn(),
      openAbout,
      setError,
    }),
  );

  result.current.onCancelLogin?.();
  result.current.onOpenAbout?.();
  result.current.onOpenCommunity?.("published-reply");
  result.current.onOpenDesktopDownload?.();
  result.current.onOpenCloudDictionary?.();
  result.current.onOpenCloudClipboard?.();

  expect(finishAccountLogin).toHaveBeenCalledOnce();
  expect(openAbout).toHaveBeenCalledOnce();
  expect(openCommunity).toHaveBeenCalledWith("published-reply");
  expect(openExternalUrl).toHaveBeenCalledOnce();
  await vi.waitFor(() => expect(setError).toHaveBeenCalledWith("无法打开云词库，请重试。"));
});

test("hides mobile-only account actions on desktop", () => {
  const { result } = renderHook(() =>
    useAccountPageActions({
      hasAccountLoginReturnPage: false,
      finishAccountLogin: vi.fn(),
      mobile: false,
      canOpenExternalUrl: true,
      openExternalUrl: vi.fn().mockResolvedValue(undefined),
      openAbout: vi.fn(),
      setError: vi.fn(),
    }),
  );

  expect(result.current.onCancelLogin).toBeUndefined();
  expect(result.current.onOpenAbout).toBeUndefined();
  expect(result.current.onOpenDesktopDownload).toBeUndefined();
  expect(result.current.onReplayOnboarding).toBeUndefined();
});
