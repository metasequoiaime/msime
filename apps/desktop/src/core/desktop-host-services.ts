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
  PluginPackage,
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
    unpublish: (id) =>
      invoke<{ deleted: boolean }>("candidate_skin_community_unpublish", {
        id,
      }),
  };
}

/**
 * The plugin-pack community on the Windows, macOS and Linux Tauri hosts.
 *
 * The host packs an installed pack from its own plugins directory and installs downloads into it, so the webview names a pack by kind and id, or a publication by its id, and never passes paths or bytes.
 */
export function createDesktopPluginCommunity(invoke: Invoke): CommunityPluginClient {
  return {
    list: (offset, search, kind) =>
      invoke<CommunityPluginPage>("plugin_community_list", {
        offset,
        search,
        kind,
      }),
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
  };
}
