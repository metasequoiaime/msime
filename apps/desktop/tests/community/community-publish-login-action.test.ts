import { expect, test, vi } from "vitest";
import { communityPublishLoginAction } from "../../../../packages/ui/src/community/community-helpers";

test("closes the publication dialog before opening login", () => {
  const events: string[] = [];
  const action = communityPublishLoginAction(
    () => events.push("close"),
    () => events.push("login"),
  );

  action?.();

  expect(events).toEqual(["close", "login"]);
});

test("does not create a login action when login is unavailable", () => {
  expect(communityPublishLoginAction(vi.fn())).toBeUndefined();
});
