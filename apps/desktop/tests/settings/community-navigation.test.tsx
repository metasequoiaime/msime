// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useCommunityGalleryNavigation } from "../../../../packages/ui/src/settings/use-community-gallery-navigation";

test("手机社区入口按类别切到社区标签", () => {
  const selectPage = vi.fn();
  const openSubview = vi.fn();
  const { result } = renderHook(() =>
    useCommunityGalleryNavigation({
      mobile: true,
      hasCommunityPage: true,
      hasSkins: true,
      hasResources: true,
      selectPage,
      openSubview,
    }),
  );

  expect(result.current.communityTabs.tabList).toBeTruthy();
  act(() => {
    result.current.openCommunityGallery({ category: "dictionary", scope: "", initialMine: false });
  });
  expect(result.current.communityTabs.selected).toBe("dictionary");
  expect(selectPage).toHaveBeenCalledWith("community");
  expect(openSubview).not.toHaveBeenCalled();
});

test("有范围或回复模板时仍在功能页打开图库", () => {
  const selectPage = vi.fn();
  const openSubview = vi.fn();
  const { result } = renderHook(() =>
    useCommunityGalleryNavigation({
      mobile: true,
      hasCommunityPage: true,
      hasSkins: true,
      hasResources: true,
      selectPage,
      openSubview,
    }),
  );

  act(() => {
    result.current.openCommunityGallery({ category: "skin", scope: "mine", initialMine: false });
    result.current.openCommunityGallery({ category: "reply", scope: "", initialMine: false });
  });
  expect(openSubview).toHaveBeenNthCalledWith(1, {
    category: "skin",
    scope: "mine",
    initialMine: false,
  });
  expect(openSubview).toHaveBeenNthCalledWith(2, {
    category: "reply",
    scope: "",
    initialMine: false,
  });
  expect(selectPage).not.toHaveBeenCalled();
});

test("没有皮肤服务时把皮肤入口落到词库图库", () => {
  const openSubview = vi.fn();
  const { result } = renderHook(() =>
    useCommunityGalleryNavigation({
      mobile: true,
      hasCommunityPage: true,
      hasSkins: false,
      hasResources: true,
      selectPage: vi.fn(),
      openSubview,
    }),
  );

  act(() => {
    result.current.openCommunityGallery({ category: "skin", scope: "mine", initialMine: false });
  });
  expect(openSubview).toHaveBeenCalledWith({
    category: "dictionary",
    scope: "mine",
    initialMine: false,
  });
});

test("桌面或没有社区页时不切标签", () => {
  for (const options of [
    { mobile: false, hasCommunityPage: true },
    { mobile: true, hasCommunityPage: false },
  ]) {
    const selectPage = vi.fn();
    const openSubview = vi.fn();
    const { result } = renderHook(() =>
      useCommunityGalleryNavigation({
        ...options,
        hasSkins: true,
        hasResources: true,
        selectPage,
        openSubview,
      }),
    );

    act(() => {
      result.current.openCommunityGallery({ category: "skin", scope: "", initialMine: false });
    });
    expect(openSubview).toHaveBeenCalledWith({ category: "skin", scope: "", initialMine: false });
    expect(selectPage).not.toHaveBeenCalled();
  }
});
