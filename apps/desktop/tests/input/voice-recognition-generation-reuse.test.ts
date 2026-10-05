import { expect, test } from "vitest";

test("voice recognition reuses the shared request generation", () => {
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

  expect(panel).toContain("const recognitionRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const recognitionRevision = useRef(0)");
});
