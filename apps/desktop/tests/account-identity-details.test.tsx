// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type IdentityDetailsProps = {
  user: {
    id: string;
    displayName: string;
    createdAt: string;
    email?: string;
  };
  providers: string[];
  copied?: boolean;
  onCopy: () => void;
};

test("account identity details share the account fields and copy action", () => {
  const Details = (ui as unknown as { AccountIdentityDetails: ComponentType<IdentityDetailsProps> })
    .AccountIdentityDetails;
  expect(Details).toBeDefined();

  const onCopy = vi.fn();
  render(
    <Details
      user={{
        id: "abcdef123456",
        displayName: "合成用户",
        createdAt: "2026-01-02T00:00:00Z",
        email: "user@example.test",
      }}
      providers={["email"]}
      onCopy={onCopy}
    />,
  );

  expect(screen.getByText("#ABCDEF")).toBeTruthy();
  expect(screen.getByText("user@example.test")).toBeTruthy();
  expect(screen.getAllByText("邮箱")).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "#ABCDEF" }));
  expect(onCopy).toHaveBeenCalledOnce();
});

test("account identity details show the copied label", () => {
  const Details = (ui as unknown as { AccountIdentityDetails: ComponentType<IdentityDetailsProps> })
    .AccountIdentityDetails;
  render(
    <Details
      user={{ id: "abcdef123456", displayName: "合成用户", createdAt: "2026-01-02T00:00:00Z" }}
      providers={[]}
      copied
      onCopy={vi.fn()}
    />,
  );

  expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy();
  expect(screen.getByText("正在读取")).toBeTruthy();
});
