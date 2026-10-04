// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useCommunityInstallState } from "../../../../packages/ui/src/community/community-install-state";

test("resets installation state when closing a community detail", () => {
  const closeGalleryDetail = vi.fn();
  const { result } = renderHook(() => useCommunityInstallState(false, closeGalleryDetail));

  act(() => {
    result.current.setInstalled(true);
    result.current.setConfirmReplace(true);
  });
  expect(result.current.installed).toBe(true);
  expect(result.current.confirmReplace).toBe(true);

  act(() => result.current.closeDetail());
  expect(closeGalleryDetail).toHaveBeenCalledOnce();
  expect(result.current.installed).toBe(false);
  expect(result.current.confirmReplace).toBe(false);
});

test("does not close while an install action is busy", () => {
  const closeGalleryDetail = vi.fn();
  const { result } = renderHook(() => useCommunityInstallState(true, closeGalleryDetail));

  act(() => result.current.closeDetail());
  expect(closeGalleryDetail).not.toHaveBeenCalled();
});
