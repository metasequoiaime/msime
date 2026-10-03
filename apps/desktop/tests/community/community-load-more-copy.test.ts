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
