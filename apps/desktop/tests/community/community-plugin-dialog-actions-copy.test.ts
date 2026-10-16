import { expect, test } from "vitest";

test("plugin publish dialogs reuse the shared dialog actions", () => {
  const dialog = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-plugin-publish-dialog.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(dialog).toContain("CommunityDialogActions");
  expect(dialog).toContain("useCommunityPluginPublication({ client, localPlugins, onPublished })");
  expect(dialog).not.toContain("./community-plugins");
  expect(page).toContain('from "./community-plugin-publish-dialog"');
  expect(page).toContain("<CommunityPluginPublishDialog");
  expect(page).not.toContain("function CommunityPluginPublishDialog(");
  expect(page).not.toContain("useCommunityPublicationDraft(");
  expect(dialog).not.toContain("<div className={style.dialogActions}>");
});
