import { expect, test } from "vitest";

test("community resources page reuses the shared client lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const page = source.slice(source.indexOf("export function CommunityResourcesPage"));

  expect(page).toContain("useCommunityClientLifecycle(");
  expect(page).not.toContain("const generation = useRef(0)");
});
