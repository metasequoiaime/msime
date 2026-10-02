import { expect, test } from "vitest";

test("resource galleries reuse the shared load-more button", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(resources).toContain("CommunityLoadMoreButton");
  expect(resources).toContain("<CommunityLoadMoreButton");
  expect(resources).not.toContain('label="加载更多"');
});
