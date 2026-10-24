import { expect, test } from "vitest";

test("MCP connection state reuses the shared async action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-mcp-connect-state.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("useMountedRef()");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
  expect(source).not.toContain("const actionRunning = useRef(false)");
});
