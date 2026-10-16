import { expect, test } from "vitest";

test("community resources page reuses the shared client lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const page = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-resource-gallery.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  expect(source).toContain("useCommunityResourceGallery(");

  expect(page).toContain("useCommunityClientLifecycle(");
  expect(page).not.toContain("const generation = useRef(0)");

  const detail = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-resource-detail.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  expect(source).toContain("<CommunityResourceDetail");
  expect(detail).toContain("useAsyncActionRunner(");
  expect(detail).not.toContain("useCommunityClientLifecycle(");
  expect(detail).not.toContain("runCommunityAction(");
  expect(detail).not.toContain("const [busy, setBusy] = useState(false)");

  const editor = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-resource-publication.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  expect(source).toContain("<CommunityResourceEditor");
  expect(editor).toContain("useAsyncActionRunner(");
  expect(editor).not.toContain("useCommunityClientLifecycle(");
  expect(editor).not.toContain("runCommunityAction(");
});
