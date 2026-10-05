import { expect, test } from "vitest";

test("community plugin pack preview reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(chosen)");
  expect(source).not.toContain("const packGeneration = useRef(0)");
  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("useCommunityClientLifecycle(");
  expect(source).not.toContain("runCommunityPublishAction(");
  expect(source).not.toContain("const [busy, setBusy] = useState(false)");
});
