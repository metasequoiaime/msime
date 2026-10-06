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

  const detailStart = source.indexOf("function ResourceDetail");
  const detailEnd = source.indexOf("export function CommunityResourcesPage", detailStart);
  const detail = source.slice(detailStart, detailEnd);
  expect(detail).toContain("useAsyncActionRunner(");
  expect(detail).not.toContain("useCommunityClientLifecycle(");
  expect(detail).not.toContain("runCommunityAction(");
  expect(detail).not.toContain("const [busy, setBusy] = useState(false)");

  const editorStart = source.indexOf("function ResourceEditor");
  const editorEnd = source.indexOf("function ResourceDetail", editorStart);
  const editor = source.slice(editorStart, editorEnd);
  expect(editor).toContain("useAsyncActionRunner(");
  expect(editor).not.toContain("useCommunityClientLifecycle(");
  expect(editor).not.toContain("runCommunityAction(");
});
