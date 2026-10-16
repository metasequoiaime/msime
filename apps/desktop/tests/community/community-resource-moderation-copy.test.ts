import { expect, test } from "vitest";

test("resource details reuse the shared moderation section", () => {
  const resources = Object.values(
    import.meta.glob<string>(
      "../../../../packages/ui/src/community/community-resource-detail.tsx",
      {
        eager: true,
        query: "?raw",
        import: "default",
      },
    ),
  )[0];

  expect(resources).toContain("CommunityModerationSection");
  expect(resources).not.toContain("CommunityRatingButtons");
  expect(resources).not.toContain("CommunityUnpublishConfirmation");
});
