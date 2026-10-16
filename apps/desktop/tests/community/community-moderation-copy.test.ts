import { expect, test } from "vitest";

test("community detail pages reuse moderation controls", () => {
  const screen = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-skin-detail.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const candidate = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-candidate-skin-detail.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  const plugin = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugin-detail.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(screen).toContain(
    'import { CommunityModerationSection } from "./community-moderation-section";',
  );
  expect(candidate).toContain(
    'import { CommunityModerationSection } from "./community-moderation-section";',
  );
  expect(plugin).toContain(
    'import { CommunityModerationSection } from "./community-moderation-section";',
  );
  expect(screen).toContain("<CommunityModerationSection");
  expect(candidate).toContain("<CommunityModerationSection");
  expect(plugin).toContain("<CommunityModerationSection");
  expect(screen).not.toContain('aria-label="我的评分"');
  expect(candidate).not.toContain('aria-label="我的评分"');
  expect(plugin).not.toContain('aria-label="我的评分"');
});
