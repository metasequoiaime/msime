import type {
  CandidateSkinCommunityClient,
  CandidateSkinPackPreview,
  CommunityCandidateSkin,
  CommunityCandidateSkinPage,
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
    list: (offset, search, mine) =>
      invoke<CommunityCandidateSkinPage>("candidate_skin_community_list", {
        offset,
        search,
        mine,
      }),
    detail: (id) => invoke<CommunityCandidateSkin>("candidate_skin_community_detail", { id }),
    preview: (id) => invoke<{ dataUrl: string }>("candidate_skin_community_preview", { id }),
    install: (id, replace) =>
      invoke<SkinCatalog>("candidate_skin_community_install", { id, replace }),
    packPreview: (skinId) =>
      invoke<CandidateSkinPackPreview>("candidate_skin_community_pack_preview", { skinId }),
    publish: (skinId, id, name, description) =>
      invoke<CommunityCandidateSkin>("candidate_skin_community_publish", {
        skinId,
        id,
        name,
        description,
      }),
    rate: (id, stars) => invoke<{ stars: number }>("candidate_skin_community_rate", { id, stars }),
    unpublish: (id) => invoke<{ deleted: boolean }>("candidate_skin_community_unpublish", { id }),
  };
}
