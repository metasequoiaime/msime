import { expect, test } from "vitest";

test("voice submission reuses the shared request generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function VoicePanel");
  const end = source.indexOf("export function CloudClipboardPanel", start);
  const panel = source.slice(start, end);

  expect(panel).toContain("const submissionRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const submissionRevision = useRef(0)");
});
