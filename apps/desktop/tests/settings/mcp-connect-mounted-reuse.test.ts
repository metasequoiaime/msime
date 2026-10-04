import { expect, test } from "vitest";

test("MCP connection section reuses the shared mounted lifecycle hook", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/mcp-connect.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useMountedRef()");
  expect(source).not.toContain("const mounted = useRef(true)");
});
