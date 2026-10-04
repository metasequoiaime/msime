import { expect, test } from "vitest";

test("provider credential tests reuse the shared action generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-provider-credentials.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const credentialTestOwner = useAsyncGeneration()");
  expect(source).not.toContain("const credentialTestOwner = useRef(0)");
});
