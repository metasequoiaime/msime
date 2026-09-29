// Synthetic, in-memory settings only. No native bridge or real user data.
import { createRoot } from "react-dom/client";
import { KeyboardPanel, SettingsPage, type Snapshot, type SkinCatalog } from "@msime/ui";
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
      candidate_page_size: 6,
      learning: true,
      chinese_punctuation: true,
    },
  };
  const root = createRoot(document.getElementById("root")!);
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
