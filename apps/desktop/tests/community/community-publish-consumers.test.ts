import { expect, test } from "vitest";

test("community skin and resource publishing reuse shared field validation", () => {
  const skins = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-skin-publication.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];
  const resources = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/use-community-resource-publication.ts",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  for (const consumer of [skins, resources]) {
    expect(consumer).toContain("communityPublishFields");
    expect(consumer).toContain(
      "const { normalizedName, normalizedDescription, nameValid, descriptionValid } =",
    );
    expect(consumer).not.toContain("const normalizedName = name.trim()");
    expect(consumer).not.toContain("const normalizedDescription = description.trim()");
  }
});
