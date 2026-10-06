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

const galleryErrorMessage = () => "请先登录";
const galleryNeedsSignIn = (value: unknown) => value === galleryFailure;
const galleryFailure = { code: "community_unauthorized" };

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

test("replaces the selected detail and matching list item together", async () => {
  const first = item("one");
  const gallery = client({
    list: vi.fn().mockResolvedValue({ items: [first], has_more: false }),
  });
  const { result } = renderHook(() => useCommunityGallery({ client: gallery }));

  await waitFor(() => expect(result.current.items).toHaveLength(1));
  act(() => result.current.open(first));
  await waitFor(() => expect(result.current.selected?.id).toBe("one"));

  act(() => result.current.replaceSelected({ ...first, name: "更新后的作品" }));

  expect(result.current.selected?.name).toBe("更新后的作品");
  expect(result.current.items[0]?.name).toBe("更新后的作品");
});

test("maps request failures and exposes when sign-in is required", async () => {
  const failure = galleryFailure;
  const gallery = client({ list: vi.fn().mockRejectedValue(failure) });
  const { result } = renderHook(() =>
    useCommunityGallery({
      client: gallery,
      errorMessage: galleryErrorMessage,
      needsSignIn: galleryNeedsSignIn,
    }),
  );

  await waitFor(() => expect(result.current.error).toBe("请先登录"));
  expect(result.current.signInRequired).toBe(true);
});

test("runs custom gallery actions through the shared busy and error lifecycle", async () => {
  const gallery = client();
  const { result } = renderHook(() =>
    useCommunityGallery({
      client: gallery,
      errorMessage: galleryErrorMessage,
      needsSignIn: galleryNeedsSignIn,
    }),
  );

  await waitFor(() => expect(gallery.list).toHaveBeenCalled());
  await act(async () => result.current.runAction(async () => undefined));
  expect(result.current.actionBusy).toBe(false);

  await act(async () =>
    result.current.runAction(async () => {
      throw galleryFailure;
    }),
  );
  expect(result.current.error).toBe("请先登录");
  expect(result.current.signInRequired).toBe(true);
});

test("ignores a duplicate gallery action while the first request is pending", async () => {
  let resolveRate!: () => void;
  const gallery = client({
    rate: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveRate = resolve;
        }),
    ),
  });
  const { result } = renderHook(() => useCommunityGallery({ client: gallery }));
  act(() => result.current.setSelected(item("one")));

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.rateSelected(5);
    second = result.current.rateSelected(5);
  });
  expect(gallery.rate).toHaveBeenCalledOnce();

  resolveRate();
  await act(async () => {
    await first;
    await second;
  });
});
