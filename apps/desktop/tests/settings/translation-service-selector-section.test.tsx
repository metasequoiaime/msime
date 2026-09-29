// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TranslationServiceSelectorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("selects a translation provider and exposes the account option when enabled", () => {
  const onChange = vi.fn();
  render(
    <TranslationServiceSelectorSection
      available
      provider="none"
      showAccountProvider
      onChange={onChange}
    />,
  );

  const select = screen.getByRole("combobox", { name: "候选词翻译服务" });
  expect(
    screen.getByRole("option", { name: "水杉账号（候选词发送到 api.msime.app）" }),
  ).toBeTruthy();
  fireEvent.change(select, { target: { value: "account" } });
  expect(onChange).toHaveBeenCalledWith("account");
});

test("disables provider selection when candidate translation is unavailable", () => {
  render(
    <TranslationServiceSelectorSection
      available={false}
      provider="tencent"
      showAccountProvider={false}
      onChange={vi.fn()}
    />,
  );

  expect(
    (screen.getByRole("combobox", { name: "候选词翻译服务" }) as HTMLSelectElement).disabled,
  ).toBe(true);
  expect(
    screen.queryByRole("option", { name: "水杉账号（候选词发送到 api.msime.app）" }),
  ).toBeNull();
});
