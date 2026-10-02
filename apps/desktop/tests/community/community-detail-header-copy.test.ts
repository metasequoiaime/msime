import { expect, test } from "vitest";

test("all community detail pages reuse the shared detail header", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const skins = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-skins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const candidates = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-candidate-skins.tsx",
      { eager: true, query: "?raw", import: "default" },
    ),
  )[0];
  const plugins = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  for (const source of [resources, skins, candidates, plugins]) {
    expect(source).toContain('import { CommunityDetailHeader } from "./community-detail-header";');
    expect(source).toContain("<CommunityDetailHeader");
    expect(source).not.toContain("className={style.detailTitle}");
  }
});
