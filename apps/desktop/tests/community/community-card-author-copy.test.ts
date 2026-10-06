import { expect, test } from "vitest";

test("resource cards reuse the shared community card author", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(resources).toContain('import { CommunityCardAuthor } from "./community-card-author";');
  expect(resources).toContain("<CommunityCardAuthor");
  expect(resources).not.toContain("className={style.cardAuthor}");
});
