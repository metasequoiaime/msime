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

  const summary = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-detail-summary.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  // 详情页可以直接用 CommunityDetailHeader，也可以用 CommunityDetailSummary：后者把标题、补充指标和状态按固定顺序排好，标题仍由 CommunityDetailHeader 画。
  expect(summary).toContain('from "./community-detail-header";');
  expect(summary).toContain("<CommunityDetailHeader");
  for (const source of [resources, skins, candidates, plugins]) {
    const direct =
      source.includes('import { CommunityDetailHeader } from "./community-detail-header";') &&
      source.includes("<CommunityDetailHeader");
    const shared =
      source.includes('import { CommunityDetailSummary } from "./community-detail-summary";') &&
      source.includes("<CommunityDetailSummary");
    expect(direct || shared).toBe(true);
    expect(source).not.toContain("className={style.detailTitle}");
  }
});
