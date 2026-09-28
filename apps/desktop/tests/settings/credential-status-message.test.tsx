// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CredentialStatusMessage } from "@msime/ui";

afterEach(cleanup);

test("announces successful credential operations as status", () => {
  render(<CredentialStatusMessage message={{ ok: true, text: "已保存合成凭据" }} />);
  expect(screen.getByRole("status").textContent).toBe("已保存合成凭据");
});

test("announces failed credential operations as alerts", () => {
  render(<CredentialStatusMessage message={{ ok: false, text: "合成凭据无效" }} />);
  expect(screen.getByRole("alert").textContent).toBe("合成凭据无效");
});

test("renders nothing before an operation has a result", () => {
  const { container } = render(<CredentialStatusMessage />);
  expect(container.firstChild).toBeNull();
});
