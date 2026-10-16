import { expect, test } from "vitest";

test("resource galleries reuse the shared load-more component", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(resources).toContain(
    'import { CommunityGalleryLoadMore } from "./community-gallery-load-more";',
  );
  expect(resources).toContain("<CommunityGalleryLoadMore");
  expect(resources).not.toContain("CommunityLoadMoreButton");
  expect(resources).not.toContain('className="secondary community-more"');
});

test("鸿蒙图库直接复用公共搜索和分页，旧皮肤页出口只做兼容转发", () => {
  const sources = import.meta.glob<string>(
    "../../../../packages/ui/src/community/{community-skins,community-resources}.tsx",
    { eager: true, query: "?raw", import: "default" },
  );
  for (const source of Object.values(sources)) {
    expect(source).toContain('import { CommunitySearchForm } from "./community-search-form";');
    expect(source).toContain('<CommunitySearchForm\n          look="harmony"');
    expect(source).toContain('<CommunityGalleryLoadMore\n          look="harmony"');
    expect(source).not.toContain("<CommunityHarmonySearch");
    expect(source).not.toContain("<CommunityHarmonyLoadMore");
    expect(source).not.toContain("function CommunityHarmonySearch(");
    expect(source).not.toContain("function CommunityHarmonyLoadMore(");
  }
  const skins = sources["../../../../packages/ui/src/community/community-skins.tsx"];
  expect(skins).toContain('export { CommunityHarmonySearch } from "./community-search-form";');
  expect(skins).toContain(
    'export { CommunityHarmonyLoadMore } from "./community-gallery-load-more";',
  );
});
