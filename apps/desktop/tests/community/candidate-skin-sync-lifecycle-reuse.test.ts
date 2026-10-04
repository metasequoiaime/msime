import { expect, test } from "vitest";

test("candidate skin sync reuses the shared client lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/candidate-skin-sync.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useCommunityClientLifecycle(");
  expect(source).not.toContain("const generation = useRef(0)");
});
