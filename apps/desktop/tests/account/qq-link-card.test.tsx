// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QqLinkCard, qqLinkMessage } from "../../../../packages/ui/src/account/qq-link-card";
import type { AccountQqLink } from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function client(initial: AccountQqLink[] = []) {
  let links = initial;
  return {
    list: vi.fn(async () => links),
    redeem: vi.fn(async (code: string) => {
      const link = { id: String(links.length + 1), createdAt: "2026-10-17T08:00:00Z" };
      links = [...links, link];
      void code;
      return link;
    }),
    remove: vi.fn(async (id: string) => {
      links = links.filter((link) => link.id !== id);
    }),
  };
}

test("a code with spaces is sent as eight digits and the new link is listed", async () => {
  const links = client();
  render(<QqLinkCard client={links} userId="u1" />);
  expect(screen.getByRole("heading", { name: "绑定 QQ 群助手" })).toBeTruthy();
  const input = await screen.findByLabelText("绑定码");
  const bind = screen.getByRole("button", { name: "绑定" }) as HTMLButtonElement;
  fireEvent.change(input, { target: { value: "1234 567" } });
  expect(bind.disabled).toBe(true);
  fireEvent.change(input, { target: { value: "1234 5678" } });
  await waitFor(() => expect(bind.disabled).toBe(false));
  fireEvent.click(bind);
  await waitFor(() => expect(links.redeem).toHaveBeenCalledWith("12345678"));
  await screen.findByText(/已绑定/);
  await screen.findByRole("button", { name: "解绑" });
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  expect(screen.queryByRole("button", { name: "绑定" })).toBeNull();
  expect(screen.queryByText(/发 \/bind/)).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "解绑" }));
  await waitFor(() => expect(links.remove).toHaveBeenCalledWith("1"));
  await screen.findByText("已解绑。");
  expect(screen.queryByRole("button", { name: "解绑" })).toBeNull();
  expect(screen.getByLabelText("绑定码")).toBeTruthy();
});

test("refusals are explained in plain words", async () => {
  const links = client();
  links.redeem.mockImplementationOnce(async () => {
    throw { code: "account_qq_link_code_not_found" };
  });
  render(<QqLinkCard client={links} userId="u1" />);
  fireEvent.change(await screen.findByLabelText("绑定码"), { target: { value: "12345678" } });
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "绑定" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "绑定" }));
  await screen.findByText(/重新发 \/bind/);
  expect(qqLinkMessage({ code: "account_qq_link_limit" })).toContain("10 个");
  expect(qqLinkMessage({ code: "account_invalid" })).toContain("8 位数字");
  expect(qqLinkMessage({ code: "account_rate_limited" })).toContain("一小时");
});

test("已有绑定时直接显示绑定状态，读取期间不闪现绑定表单", async () => {
  const links = client([{ id: "1", createdAt: "2026-10-17T08:00:00Z" }]);
  render(<QqLinkCard client={links} userId="u1" />);
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  await screen.findByRole("button", { name: "解绑" });
  expect(screen.getByRole("heading", { name: "QQ 群助手" })).toBeTruthy();
  expect(screen.getByText(/已绑定。在 QQ 群里/)).toBeTruthy();
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  expect(screen.queryByRole("button", { name: "绑定" })).toBeNull();
  expect(screen.queryByText(/发 \/bind/)).toBeNull();
});

test("绑定其他 QQ 需要主动展开，取消或绑定成功后收起表单", async () => {
  const links = client([{ id: "1", createdAt: "2026-10-17T08:00:00Z" }]);
  render(<QqLinkCard client={links} userId="u1" />);
  fireEvent.click(await screen.findByRole("button", { name: "绑定其他 QQ" }));
  fireEvent.change(screen.getByLabelText("绑定码"), { target: { value: "12345678" } });
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  expect(links.redeem).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "绑定其他 QQ" }));
  expect((screen.getByLabelText("绑定码") as HTMLInputElement).value).toBe("");
  fireEvent.change(screen.getByLabelText("绑定码"), { target: { value: "12345678" } });
  fireEvent.click(screen.getByRole("button", { name: "绑定" }));
  await screen.findByText("绑定成功。");
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  expect(screen.getAllByRole("button", { name: "解绑" })).toHaveLength(2);
  fireEvent.click(screen.getAllByRole("button", { name: "解绑" })[0]!);
  await waitFor(() => expect(screen.getAllByRole("button", { name: "解绑" })).toHaveLength(1));
  expect(screen.queryByLabelText("绑定码")).toBeNull();
});

test("读取失败不当作未绑定，重试成功后按实际状态展示", async () => {
  const links = client([{ id: "1", createdAt: "2026-10-17T08:00:00Z" }]);
  links.list.mockRejectedValueOnce({ code: "account_unavailable" });
  render(<QqLinkCard client={links} userId="u1" />);
  fireEvent.click(await screen.findByRole("button", { name: "重试" }));
  expect(screen.queryByLabelText("绑定码")).toBeNull();
  await screen.findByRole("button", { name: "解绑" });
  expect(links.list).toHaveBeenCalledTimes(2);
  expect(screen.queryByLabelText("绑定码")).toBeNull();
});

test.each(["client", "user"] as const)(
  "切换 %s 后旧绑定成功不能清空新草稿或继续刷新",
  async (owner) => {
    const pending = deferred<AccountQqLink>();
    const previous = client();
    previous.redeem.mockReturnValueOnce(pending.promise);
    const next = owner === "client" ? client() : previous;
    const view = render(<QqLinkCard client={previous} userId="synthetic-old" />);
    fireEvent.change(await screen.findByLabelText("绑定码"), { target: { value: "12345678" } });
    fireEvent.click(screen.getByRole("button", { name: "绑定" }));
    await waitFor(() => expect(previous.redeem).toHaveBeenCalledOnce());
    view.rerender(
      <QqLinkCard client={next} userId={owner === "user" ? "synthetic-new" : "synthetic-old"} />,
    );
    const input = await screen.findByLabelText("绑定码");
    fireEvent.change(input, { target: { value: "87654321" } });
    const reads = previous.list.mock.calls.length;
    await act(async () => {
      pending.resolve({ id: "synthetic-link", createdAt: "2026-10-17T08:00:00Z" });
    });
    expect((screen.getByLabelText("绑定码") as HTMLInputElement).value).toBe("87654321");
    expect(previous.list).toHaveBeenCalledTimes(reads);
    expect(screen.queryByText("绑定成功。")).toBeNull();
  },
);

test.each(["client", "user"] as const)(
  "切换 %s 后旧绑定失败不能覆盖新账号正在处理的状态",
  async (owner) => {
    const pending = deferred<AccountQqLink>();
    const current = deferred<AccountQqLink>();
    const previous = client();
    previous.redeem.mockReturnValueOnce(pending.promise);
    const next = owner === "client" ? client() : previous;
    next.redeem.mockReturnValueOnce(current.promise);
    const view = render(<QqLinkCard client={previous} userId="synthetic-old" />);
    fireEvent.change(await screen.findByLabelText("绑定码"), { target: { value: "12345678" } });
    fireEvent.click(screen.getByRole("button", { name: "绑定" }));
    await waitFor(() => expect(previous.redeem).toHaveBeenCalledWith("12345678"));
    view.rerender(
      <QqLinkCard client={next} userId={owner === "user" ? "synthetic-new" : "synthetic-old"} />,
    );
    fireEvent.change(await screen.findByLabelText("绑定码"), { target: { value: "87654321" } });
    fireEvent.click(screen.getByRole("button", { name: "绑定" }));
    await waitFor(() => expect(next.redeem).toHaveBeenCalledWith("87654321"));
    await act(async () => {
      pending.reject({ code: "account_qq_link_code_not_found" });
    });
    expect(screen.queryByText(/绑定码不对或已过期/)).toBeNull();
    expect((screen.getByRole("button", { name: "绑定" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("正在处理…")).toBeTruthy();
    await act(async () => {
      current.resolve({ id: "synthetic-current", createdAt: "2026-10-17T08:00:00Z" });
    });
    expect(await screen.findByText("绑定成功。")).toBeTruthy();
  },
);

test.each(["绑定", "解绑"] as const)(
  "%s 后的列表读取跨账号返回时不能收起新账号的表单或显示旧提示",
  async (operation) => {
    const previous = client([{ id: "synthetic-old", createdAt: "2026-10-17T08:00:00Z" }]);
    const pending = deferred<AccountQqLink[]>();
    const view = render(<QqLinkCard client={previous} userId="synthetic-old" />);
    await screen.findByRole("button", { name: "绑定其他 QQ" });
    previous.list.mockReturnValueOnce(pending.promise);
    if (operation === "绑定") {
      fireEvent.click(screen.getByRole("button", { name: "绑定其他 QQ" }));
      fireEvent.change(screen.getByLabelText("绑定码"), { target: { value: "12345678" } });
    }
    fireEvent.click(screen.getByRole("button", { name: operation }));
    await waitFor(() => expect(previous.list).toHaveBeenCalledTimes(2));
    previous.list.mockResolvedValueOnce([
      { id: "synthetic-new", createdAt: "2026-10-18T08:00:00Z" },
    ]);
    view.rerender(<QqLinkCard client={previous} userId="synthetic-new" />);
    fireEvent.click(await screen.findByRole("button", { name: "绑定其他 QQ" }));
    fireEvent.change(screen.getByLabelText("绑定码"), { target: { value: "87654321" } });
    await act(async () => {
      pending.resolve([]);
    });
    expect((screen.getByLabelText("绑定码") as HTMLInputElement).value).toBe("87654321");
    expect(screen.getByRole("button", { name: "取消" })).toBeTruthy();
    expect(screen.queryByText("绑定成功。")).toBeNull();
    expect(screen.queryByText("已解绑。")).toBeNull();
  },
);

test("切换客户端后旧解绑成功不继续读取旧客户端或污染新提示", async () => {
  const pending = deferred<void>();
  const previous = client([{ id: "synthetic-old", createdAt: "2026-10-17T08:00:00Z" }]);
  previous.remove.mockReturnValueOnce(pending.promise);
  const next = client();
  const view = render(<QqLinkCard client={previous} userId="synthetic-old" />);
  fireEvent.click(await screen.findByRole("button", { name: "解绑" }));
  await waitFor(() => expect(previous.remove).toHaveBeenCalledOnce());
  view.rerender(<QqLinkCard client={next} userId="synthetic-new" />);
  await screen.findByLabelText("绑定码");
  await act(async () => {
    pending.resolve();
  });
  expect(previous.list).toHaveBeenCalledOnce();
  expect(screen.queryByText("已解绑。")).toBeNull();
});
