// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useCommunityGallery,
  type CommunityGalleryClient,
} from "../../../../packages/ui/src/community/community-gallery";

afterEach(() => {
  vi.restoreAllMocks();
});

type Item = { id: string; name: string };

function item(id: string): Item {
  return { id, name: `作品 ${id}` };
}

function client(
  overrides: Partial<CommunityGalleryClient<Item>> = {},
): CommunityGalleryClient<Item> {
  return {
    list: vi.fn().mockResolvedValue({ items: [], has_more: false }),
    detail: vi.fn().mockImplementation(async (id) => item(id)),
    rate: vi.fn().mockResolvedValue(undefined),
    unpublish: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

test("loads pages, deduplicates appended items, and keeps the active search", async () => {
  const gallery = client({
    list: vi
      .fn()
      .mockImplementation(async (_offset, query, _mine) =>
        query === "needle"
          ? _offset === 0
            ? { items: [item("one"), item("two")], has_more: true }
            : { items: [item("two"), item("three")], has_more: false }
          : { items: [], has_more: false },
      ),
  });
  const { result } = renderHook(() => useCommunityGallery({ client: gallery }));

  await waitFor(() => expect(gallery.list).toHaveBeenCalledWith(0, "", false));
  await act(async () => {
    await result.current.requestList("needle", false);
    await result.current.requestList("needle", true);
  });

  expect(gallery.list).toHaveBeenCalledWith(0, "needle", false);
  expect(gallery.list).toHaveBeenCalledWith(2, "needle", false);
  expect(result.current.items.map((value) => value.id)).toEqual(["one", "two", "three"]);
  expect(result.current.activeSearch).toBe("needle");
});

test("refreshes selected detail after rating and clears it after unpublishing", async () => {
  const first = item("one");
  const gallery = client({
    list: vi.fn().mockResolvedValue({ items: [first], has_more: false }),
    detail: vi.fn().mockResolvedValue({ ...first, name: "详情" }),
  });
  const { result } = renderHook(() => useCommunityGallery({ client: gallery }));

  await waitFor(() => expect(result.current.items).toHaveLength(1));
  act(() => result.current.open(first));
  await waitFor(() => expect(result.current.selected?.name).toBe("详情"));
  await act(async () => result.current.rateSelected(5));

  expect(gallery.rate).toHaveBeenCalledWith("one", 5);
  expect(result.current.actionNotice).toBe("已评分：5 星。");

  await act(async () => result.current.unpublishSelected("已下架。"));
  expect(gallery.unpublish).toHaveBeenCalledWith("one");
  expect(result.current.selected).toBeNull();
  expect(result.current.actionNotice).toBe("已下架。");
});

test("maps request failures and exposes when sign-in is required", async () => {
  const failure = { code: "auth_required" };
  const gallery = client({ list: vi.fn().mockRejectedValue(failure) });
  const { result } = renderHook(() =>
    useCommunityGallery({
      client: gallery,
      errorMessage: () => "请先登录",
      needsSignIn: (value) => value === failure,
    }),
  );

  await waitFor(() => expect(result.current.error).toBe("请先登录"));
  expect(result.current.signInRequired).toBe(true);
});
