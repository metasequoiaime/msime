import { expect, test } from "vitest";

test("cloud candidates input reuses the shared latest ref", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function CloudCandidatesPanel");
  const end = source.indexOf("export function EmojiPanel", start);
  const panel = source.slice(start, end);

  expect(panel).toContain("const textRef = useLatestRef(text)");
  expect(panel).not.toContain('const textRef = useRef("")');
});
