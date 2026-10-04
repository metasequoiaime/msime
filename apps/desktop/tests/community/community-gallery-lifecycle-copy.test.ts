import { expect, test } from "vitest";

test("community galleries reuse the shared client lifecycle guard", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-gallery.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useCommunityClientLifecycle");
});
