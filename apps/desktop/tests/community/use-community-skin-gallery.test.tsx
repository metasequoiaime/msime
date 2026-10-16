// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { useCommunitySkinGallery, type CommunitySkin, type CommunitySkinClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const design = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0 as const,
  monospaced: false,
};

const skin = (id: string): CommunitySkin => ({
  id,
  name: "示例皮肤",
  description: "说明",
  author: "示例作者",
  design,
  downloads: 1,
  rating_count: 0,
  rating_average: 0,
  owned: false,
  my_rating: 0,
});

const client = (overrides: Partial<CommunitySkinClient> = {}): CommunitySkinClient => ({
  list: vi.fn().mockResolvedValue({ skins: [skin("skin-1")], has_more: false }),
  detail: vi.fn().mockResolvedValue(skin("skin-1")),
  download: vi
    .fn()
    .mockResolvedValue({ skin: skin("skin-1"), trial: { id: "trial-1", name: "示例皮肤" } }),
  rate: vi.fn().mockResolvedValue(undefined),
  publish: vi.fn().mockResolvedValue(undefined),
  unpublish: vi.fn().mockResolvedValue(undefined),
  setCategory: vi.fn().mockResolvedValue(skin("skin-1")),
  finishTrial: vi.fn().mockResolvedValue(undefined),
  ...overrides,
});

test("loads skins and exposes the active search and category controls", async () => {
  const community = client();
  const { result } = renderHook(() => useCommunitySkinGallery({ client: community }));

  await waitFor(() => expect(community.list).toHaveBeenCalledWith(0, "", false, null));
  await waitFor(() => expect(result.current.items).toHaveLength(1));

  await result.current.requestList("森林", false);
  expect(community.list).toHaveBeenLastCalledWith(0, "森林", false, null);
});

test("finishes an active trial when the gallery closes", async () => {
  const community = client({
    download: vi.fn().mockResolvedValue({
      skin: skin("skin-1"),
      trial: { id: "trial-2", name: "示例皮肤" },
    }),
  });
  const { result } = renderHook(() => useCommunitySkinGallery({ client: community }));
  await waitFor(() => expect(community.list).toHaveBeenCalled());
  result.current.open(skin("skin-1"));
  await waitFor(() => expect(result.current.selected?.id).toBe("skin-1"));
  await result.current.download();
  await waitFor(() => expect(result.current.trial?.id).toBe("trial-2"));
  await result.current.closeDetail();
  await waitFor(() => expect(community.finishTrial).toHaveBeenCalledWith("trial-2", false));
});
