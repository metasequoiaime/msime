// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QqLinkCard, qqLinkMessage } from "../../../../packages/ui/src/account/qq-link-card";
import type { AccountQqLink } from "@msime/ui";

afterEach(cleanup);

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
  const input = screen.getByLabelText("绑定码");
  const bind = screen.getByRole("button", { name: "绑定" }) as HTMLButtonElement;
  fireEvent.change(input, { target: { value: "1234 567" } });
  expect(bind.disabled).toBe(true);
  fireEvent.change(input, { target: { value: "1234 5678" } });
  await waitFor(() => expect(bind.disabled).toBe(false));
  fireEvent.click(bind);
  await waitFor(() => expect(links.redeem).toHaveBeenCalledWith("12345678"));
  await screen.findByText(/已绑定/);
  await screen.findByRole("button", { name: "解绑" });

  fireEvent.click(screen.getByRole("button", { name: "解绑" }));
  await waitFor(() => expect(links.remove).toHaveBeenCalledWith("1"));
  await screen.findByText("已解绑。");
  expect(screen.queryByRole("button", { name: "解绑" })).toBeNull();
});

test("refusals are explained in plain words", async () => {
  const links = client();
  links.redeem.mockImplementationOnce(async () => {
    throw { code: "account_qq_link_code_not_found" };
  });
  render(<QqLinkCard client={links} userId="u1" />);
  fireEvent.change(screen.getByLabelText("绑定码"), { target: { value: "12345678" } });
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
