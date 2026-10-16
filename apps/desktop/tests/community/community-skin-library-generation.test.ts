import { expect, test } from "vitest";

test("社区皮肤库的实际持有者复用异步请求代次，页面委托完整库操作", () => {
  const sources = import.meta.glob<string>(
    "../../../../packages/ui/src/community/{community-skins.tsx,use-community-skin-library.ts}",
    {
      eager: true,
      query: "?raw",
      import: "default",
    },
  );
  const source = sources["../../../../packages/ui/src/community/use-community-skin-library.ts"];
  const page = sources["../../../../packages/ui/src/community/community-skins.tsx"];
  expect(source).toContain("useAsyncGeneration(harmony, browsing, localSkinLibrary)");
  expect(source).not.toContain("let current = true");
  expect(page).toContain("const { pillState, take, apply } = useCommunitySkinLibrary({");
  expect(page).toContain("browsing: selected === null");
  expect(page).not.toContain("const libraryGeneration = useRef(0)");
});
