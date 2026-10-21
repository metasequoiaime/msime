import { createCommunityResourceClient } from "./community-resource-host";
import { createSettingsSyncClient } from "./settings-sync-host";

import type {
  CandidateSkinCommunityClient,
  CandidateSkinPackPreview,
  CandidateSkinSyncReport,
  CommunityCandidateSkin,
  CommunityCandidateSkinPage,
  CommunityPlugin,
  CommunityPluginClient,
  CommunityPluginPackPreview,
  CommunityPluginPage,
  CommunityResourceClient,
  PluginPackage,
  SettingsSyncClient,
  SkinCatalog,
} from "@msime/ui";

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/**
 * The candidate-window skin community on the Windows, macOS and Linux Tauri hosts.
 *
 * The host packs a package from its own skin directory and installs downloads into it, so the webview names a folder or a publication and never passes paths or bytes.
 */
export function createDesktopCandidateSkinCommunity(invoke: Invoke): CandidateSkinCommunityClient {
  return {
    list: (offset, search, mine, category) =>
      invoke<CommunityCandidateSkinPage>("candidate_skin_community_list", {
        offset,
        search,
        mine,
        category,
      }),
    detail: (id) => invoke<CommunityCandidateSkin>("candidate_skin_community_detail", { id }),
    preview: (id) => invoke<{ dataUrl: string }>("candidate_skin_community_preview", { id }),
    install: (id, replace) =>
      invoke<SkinCatalog>("candidate_skin_community_install", { id, replace }),
    packPreview: (skinId, visibility) =>
      invoke<CandidateSkinPackPreview>("candidate_skin_community_pack_preview", {
        skinId,
        visibility,
      }),
    addPreview: (skinId, bytes) =>
      invoke<SkinCatalog>("candidate_skin_community_add_preview", {
        skinId,
        bytes,
      }),
    addLicense: (skinId, assets) =>
      invoke<SkinCatalog>("candidate_skin_community_add_license", {
        skinId,
        assets,
      }),
    publish: (skinId, id, name, description, visibility, category) =>
      invoke<CommunityCandidateSkin>("candidate_skin_community_publish", {
        skinId,
        id,
        name,
        description,
        visibility,
        category,
      }),
    setVisibility: (id, visibility) =>
      invoke<CommunityCandidateSkin>("candidate_skin_community_set_visibility", {
        id,
        visibility,
      }),
    setCategory: (id, category) =>
      invoke<CommunityCandidateSkin>("candidate_skin_community_set_category", {
        id,
        category,
      }),
    sync: () => invoke<CandidateSkinSyncReport>("candidate_skin_community_sync"),
    rate: (id, stars) => invoke<{ stars: number }>("candidate_skin_community_rate", { id, stars }),
    unpublish: (id) => invoke<{ deleted: boolean }>("candidate_skin_community_unpublish", { id }),
    report: (id, reason, detail) =>
      invoke<void>("community_report", { kind: "candidate-skins", id, reason, detail }),
  };
}

/**
 * The plugin-pack community on the Windows, macOS and Linux Tauri hosts.
 *
 * The host packs an installed pack from its own plugins directory and installs downloads into it, so the webview names a pack by kind and id, or a publication by its id, and never passes paths or bytes.
 */
export function createDesktopPluginCommunity(invoke: Invoke): CommunityPluginClient {
  return {
    list: (offset, search, kind, mine) =>
      invoke<CommunityPluginPage>("plugin_community_list", { offset, search, kind, mine }),
    detail: (id) => invoke<CommunityPlugin>("plugin_community_detail", { id }),
    packPreview: (kind, pluginId) =>
      invoke<CommunityPluginPackPreview>("plugin_community_pack_preview", {
        kind,
        pluginId,
      }),
    publish: (kind, pluginId, id, name, description) =>
      invoke<CommunityPlugin>("plugin_community_publish", {
        kind,
        pluginId,
        id,
        name,
        description,
      }),
    install: (id, kind, pluginId) =>
      invoke<PluginPackage>("plugin_community_install", { id, kind, pluginId }),
    rate: (id, stars) => invoke<{ stars: number }>("plugin_community_rate", { id, stars }),
    delete: (id) => invoke<{ deleted: boolean }>("plugin_community_delete", { id }),
    report: (id, reason, detail) =>
      invoke<void>("community_report", { kind: "plugins", id, reason, detail }),
  };
}

/**
 * 社区词包与回复模板，接在 Windows 和 Linux 设置应用的账号会话上，命令与移动端同名。
 *
 * 桌面没有「高情商回复」键盘，所以不提供 storeReply / removeReply：回复模板只能收藏、评分、举报和发布，词包导入本机词库或账号云端词库。
 */
export function createDesktopCommunityResources(invoke: Invoke): CommunityResourceClient {
  return createCommunityResourceClient(invoke);
}

/**
 * 设置同步卡片的四个命令，与移动端同名。Windows 和 Linux 只上传、应用各平台共有的输入设置（方案、繁简、双拼和五笔版本、学习、调频和标点）。
 */
export function createDesktopSettingsSync(invoke: Invoke): SettingsSyncClient {
  return {
    ...createSettingsSyncClient(invoke),
    description:
      "同步输入方案、繁体输出、双拼和五笔版本、词库学习、调频和标点。凭据、联网授权及输入内容不会随设置上传。",
    // 设置存进共享偏好后 Windows 的 Server、Linux 的 IBus 和 Fcitx5 宿主当场读到，没有要重新打开的键盘。
    appliedMessage: "已应用云端设置。",
  };
}
