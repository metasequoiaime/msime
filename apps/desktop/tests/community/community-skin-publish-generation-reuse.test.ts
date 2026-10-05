import { expect, test } from "vitest";

test("community skin publish loading reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-skins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("clientGeneration.current");
  expect(source).not.toContain("let active = true");
  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("useCommunityClientLifecycle(");
  expect(source).not.toContain("runCommunityPublishAction(");
});
