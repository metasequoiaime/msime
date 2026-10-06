// Synthetic, in-memory settings only. No native bridge or real user data.
import { createRoot } from "react-dom/client";
import {
  KeyboardPanel,
  SettingsPage,
  type CandidateSkinCommunityClient,
  type Snapshot,
  type SkinCatalog,
} from "@msime/ui";
import "../../../../packages/ui/src/styles.css";

export function mountKeyboard(theme: "dark" | "light" = "dark") {
  const root = createRoot(document.getElementById("root")!);
  root.render(<KeyboardPanel theme={theme} client={{ close: async () => {} }} />);
  return () => root.unmount();
}

export function mount() {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      wubi_profile: "wubi86",
      candidate_page_size: 6,
      learning: true,
      chinese_punctuation: true,
    },
  };
  const root = createRoot(document.getElementById("root")!);
  // The candidate-skin community has no server here: the gallery is empty, the packer answers for the synthetic package, and anything that would reach the network fails as an unavailable community.
  const unavailable = async (): Promise<never> => {
    throw { code: "community_unavailable" };
  };
  const communityCandidateSkins: CandidateSkinCommunityClient = {
    list: async () => ({ skins: [], has_more: false }),
    detail: unavailable,
    preview: unavailable,
    install: unavailable,
    packPreview: async () => ({
      suggestedName: "Synthetic external",
      license: { code: null, assets: "CC0-1.0", source: null },
      fileCount: 1,
      size: 2048,
    }),
    addPreview: unavailable,
    addLicense: unavailable,
    publish: unavailable,
    rate: unavailable,
    unpublish: unavailable,
    setVisibility: unavailable,
    setCategory: unavailable,
    sync: unavailable,
  };
  const catalog: SkinCatalog = {
    directory: "/synthetic/skins",
    issues: [],
    packages: [
      {
        id: "sample",
        name: "Synthetic external",
        version: "1",
        base: "system",
        author: null,
        description: null,
        layouts: ["horizontal", "vertical"],
        themes: ["dark", "light"],
        minWidthDip: 100,
        decorationTopDip: 24,
        decorationWidthDip: 100,
        toolbarStylesheet: null,
        preview: "sample.svg",
        candidate: {
          dark: { surface: "#123456", border: "#112233", showSelectedBar: false },
          light: { surface: "#abcdef" },
        },
      },
    ],
  };
  root.render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: async (_revision, preferences) => ({ ...snapshot, preferences }),
        scanSkinCatalog: async () => catalog,
        communityCandidateSkins,
        listFontFamilies: async () => ["Segoe UI", "示例字体", "加倍示例", "缺字示例"],
        readSkinImage: async () => ({
          contentType: "image/svg+xml",
          bytes: [
            ...new TextEncoder().encode(
              '<svg xmlns="http://www.w3.org/2000/svg" width="100" height="24"><rect width="100" height="24" fill="#abcdef"/></svg>',
            ),
          ],
        }),
      }}
    />,
  );
  return () => root.unmount();
}
