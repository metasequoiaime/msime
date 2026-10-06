import { expect, test } from "vitest";

test("community publication dialogs reuse the shared metadata fields", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const plugins = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const skins = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-skin-publication-fields.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  for (const source of [resources, plugins, skins]) {
    expect(source).toContain("import { CommunityPublicationMetadataFields }");
    expect(source).toContain("<CommunityPublicationMetadataFields");
  }
});
