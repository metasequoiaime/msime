import { expect, test, vi } from "vitest";
import {
  createDesktopCandidateSkinCommunity,
  createDesktopCommunityResources,
  createDesktopPluginCommunity,
  createDesktopSettingsSync,
} from "../../src/core/desktop-host-services";

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
  await community.report?.(id, "侵权/抄袭", "抄袭了我的作品");

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
    [
      "community_report",
      { kind: "candidate-skins", id, reason: "侵权/抄袭", detail: "抄袭了我的作品" },
    ],
  ]);
});

test("the desktop plugin community lists the user's own packs and reports with the plugins kind", async () => {
  const invoke = vi.fn(async () => undefined);
  const community = createDesktopPluginCommunity(
    invoke as unknown as Parameters<typeof createDesktopPluginCommunity>[0],
  );
  const id = "10000000-0000-4000-8000-000000000002";

  await community.list(0, "", "sound", true);
  await community.report?.(id, "恶意插件", "");

  expect(invoke.mock.calls).toEqual([
    ["plugin_community_list", { offset: 0, search: "", kind: "sound", mine: true }],
    ["community_report", { kind: "plugins", id, reason: "恶意插件", detail: "" }],
  ]);
});

test("Windows community resources use the shared commands and leave the reply keyboard out", async () => {
  const invoke = vi.fn(async () => undefined);
  const resources = createDesktopCommunityResources(
    invoke as unknown as Parameters<typeof createDesktopCommunityResources>[0],
  );
  const id = "10000000-0000-4000-8000-000000000003";

  await resources.list("dictionary", "mine", "成语", 20);
  await resources.detail(id);
  await resources.publish(id, "reply", "婉拒", "礼貌回绝", { prompt: "礼貌地拒绝" }, 2);
  await resources.apply(id, 3);
  await resources.save(id, true);
  await resources.rate(id, 4);
  await resources.unpublish(id);
  await resources.report?.("reply", id, "垃圾广告", "");

  // 桌面没有「高情商回复」键盘，页面据此不显示添加到键盘的按钮。
  expect(resources.storeReply).toBeUndefined();
  expect(resources.removeReply).toBeUndefined();
  expect(invoke.mock.calls).toEqual([
    ["community_resource_list", { kind: "dictionary", scope: "mine", search: "成语", offset: 20 }],
    ["community_resource_detail", { id }],
    [
      "community_resource_publish",
      {
        id,
        kind: "reply",
        name: "婉拒",
        description: "礼貌回绝",
        content: { prompt: "礼貌地拒绝" },
        revision: 2,
      },
    ],
    ["community_resource_apply", { id, resourceRevision: 3 }],
    ["community_resource_save", { id, saved: true }],
    ["community_resource_rate", { id, stars: 4 }],
    ["community_resource_unpublish", { id }],
    ["community_report", { kind: "replies", id, reason: "垃圾广告", detail: "" }],
  ]);
});

test("Windows settings sync invokes the same four commands as the mobile hosts", async () => {
  const invoke = vi.fn(async () => undefined);
  const sync = createDesktopSettingsSync(
    invoke as unknown as Parameters<typeof createDesktopSettingsSync>[0],
  );
  const preferences = { revision: 3, settings: { "input.learning": true } };

  await sync.schema();
  await sync.load();
  await sync.upload();
  await sync.apply("synthetic-user", preferences);

  expect(invoke.mock.calls).toEqual([
    ["account_preferences_schema"],
    ["account_preferences_load"],
    ["account_preferences_upload"],
    ["account_preferences_apply", { userId: "synthetic-user", preferences }],
  ]);
  // Windows 没有要重新打开的键盘，应用后的提示不能沿用移动端的说法。
  expect(sync.appliedMessage).toBe("已应用云端设置。");
});
