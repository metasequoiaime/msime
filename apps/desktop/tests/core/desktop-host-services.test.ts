import { expect, test, vi } from "vitest";
import { createDesktopCandidateSkinCommunity } from "../../src/core/desktop-host-services";

test("the desktop candidate-skin community invokes the host commands with camelCase args", async () => {
  const invoke = vi.fn(async () => undefined);
  const community = createDesktopCandidateSkinCommunity(
    invoke as unknown as Parameters<typeof createDesktopCandidateSkinCommunity>[0],
  );
  const id = "10000000-0000-4000-8000-000000000001";

  await community.list(20, "水墨", true, "guofeng");
  await community.detail(id);
  await community.preview(id);
  await community.install(id, true);
  await community.packPreview("ink-wash", "private");
  await community.publish("ink-wash", id, "水墨", "淡墨", "private", "minimal");
  await community.rate(id, 5);
  await community.unpublish(id);
  await community.setVisibility(id, "public");
  await community.setCategory(id, "food");
  await community.sync();

  expect(invoke.mock.calls).toEqual([
    [
      "candidate_skin_community_list",
      { offset: 20, search: "水墨", mine: true, category: "guofeng" },
    ],
    ["candidate_skin_community_detail", { id }],
    ["candidate_skin_community_preview", { id }],
    ["candidate_skin_community_install", { id, replace: true }],
    ["candidate_skin_community_pack_preview", { skinId: "ink-wash", visibility: "private" }],
    [
      "candidate_skin_community_publish",
      {
        skinId: "ink-wash",
        id,
        name: "水墨",
        description: "淡墨",
        visibility: "private",
        category: "minimal",
      },
    ],
    ["candidate_skin_community_rate", { id, stars: 5 }],
    ["candidate_skin_community_unpublish", { id }],
    ["candidate_skin_community_set_visibility", { id, visibility: "public" }],
    ["candidate_skin_community_set_category", { id, category: "food" }],
    ["candidate_skin_community_sync"],
  ]);
});
