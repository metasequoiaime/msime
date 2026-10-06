// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountAvatar } from "../../../../packages/ui/src/account/account-avatar";
import type { AccountUser } from "../../../../packages/ui/src/account/account-page";

afterEach(cleanup);

function avatarUser(avatarUrl: string): AccountUser {
  return {
    id: "synthetic-avatar-cache-user",
    displayName: "合成头像用户",
    createdAt: "2026-01-01T00:00:00Z",
    avatarUrl,
  };
}

test("缓存达到容量后淘汰最旧头像", async () => {
  const load = vi.fn(async () => "data:image/png;base64,c3ludGhldGlj");
  const { rerender } = render(
    <AccountAvatar user={avatarUser("synthetic-avatar-0")} load={load} size="small" />,
  );

  for (let index = 1; index <= 16; index++) {
    await act(async () => {
      rerender(
        <AccountAvatar user={avatarUser(`synthetic-avatar-${index}`)} load={load} size="small" />,
      );
      await Promise.resolve();
    });
  }
  await act(async () => {
    rerender(<AccountAvatar user={avatarUser("synthetic-avatar-0")} load={load} size="small" />);
    await Promise.resolve();
  });

  expect(load).toHaveBeenCalledTimes(18);
});

test("迟到的旧头像失败不会删除同 URL 的新请求", async () => {
  let rejectFirst!: (reason?: unknown) => void;
  const first = new Promise<string | null>((_resolve, reject) => {
    rejectFirst = reject;
  });
  const replacement = new Promise<string | null>(() => {});
  const load = vi
    .fn<() => Promise<string | null>>()
    .mockImplementationOnce(() => first)
    .mockImplementationOnce(() => replacement)
    .mockResolvedValue("data:image/png;base64,c3ludGhldGlj");
  const view = (avatarUrl: string) => (
    <AccountAvatar user={avatarUser(avatarUrl)} load={load} size="small" />
  );
  const { rerender, unmount } = render(view("synthetic-avatar-race-0"));

  for (let index = 1; index <= 16; index++) {
    await act(async () => {
      rerender(view(`synthetic-avatar-race-${index}`));
      await Promise.resolve();
    });
  }
  await act(async () => {
    rerender(view("synthetic-avatar-race-0"));
    await Promise.resolve();
  });
  await act(async () => {
    rejectFirst(new Error("synthetic avatar failure"));
    await Promise.resolve();
  });
  unmount();
  render(view("synthetic-avatar-race-0"));

  expect(load).toHaveBeenCalledTimes(18);
});
