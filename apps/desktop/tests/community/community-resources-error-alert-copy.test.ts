import { expect, test } from "vitest";

test("resource pages reuse the shared error alert", () => {
  const resources = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(resources).toContain("CommunityErrorAlert");
  expect(resources).not.toContain('<p role="alert" className="error">');
});
