import { expect, test } from "vitest";

test("plugin publish dialogs reuse the shared dialog actions", () => {
  const plugins = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(plugins).toContain("CommunityDialogActions");
  expect(plugins).not.toContain("<div className={style.dialogActions}>");
});
