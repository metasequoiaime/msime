import { expect, test } from "vitest";

test("MCP settings reuse the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/mcp-connect.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(status, install, copyText)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
  expect(source).toContain(
    "const refreshGeneration = useAsyncGeneration(status, install, copyText)",
  );
  expect(source).not.toContain("const refreshGeneration = useRef(0)");
});
