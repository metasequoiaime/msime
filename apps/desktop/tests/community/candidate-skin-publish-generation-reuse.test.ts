import { expect, test } from "vitest";

test("candidate skin publish loading reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/candidate-skin-publish-dialog.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(source).toContain("clientGeneration.current");
  expect(source).not.toContain("let active = true");
  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("useCommunityClientLifecycle(");
  expect(source).not.toContain("runCommunityPublishAction(");
  expect(source).not.toContain("const [busy, setBusy] = useState(false)");
});
