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

test("grouped selector keeps the settings row layout", () => {
  const onChange = vi.fn();
  render(
    <TranslationServiceSelectorSection
      grouped
      available
      provider="none"
      showAccountProvider={false}
      onChange={onChange}
    />,
  );

  expect(screen.getByRole("group", { name: "候选词翻译服务" })).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "候选词翻译服务" }), {
    target: { value: "tencent" },
  });
  expect(onChange).toHaveBeenCalledWith("tencent");
});

test("macOS names the no-service choice offline translation and says where glosses come from", () => {
  const { rerender } = render(
    <TranslationServiceSelectorSection
      grouped
      available
      provider="none"
      showAccountProvider={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.getByRole("option", { name: "关闭" })).toBeTruthy();
  expect(screen.queryByText(/随输入法打包的离线词库/)).toBeNull();

  rerender(
    <TranslationServiceSelectorSection
      grouped
      available
      provider="none"
      showAccountProvider={false}
      offlineFallback="system"
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("option", { name: "关闭" })).toBeNull();
  const offline = screen.getByRole("option", { name: "离线翻译（不联网）" }) as HTMLOptionElement;
  expect(offline.value).toBe("none");
  expect(screen.getByText(/随输入法打包的离线词库/)).toBeTruthy();
});

test("hosts with only offline dictionaries name the no-service choice 不使用在线服务", () => {
  // Windows、Linux、HarmonyOS 没有系统翻译，「关闭」又不对：host-api 仍把装好的离线词库语言放进查询。
  render(
    <TranslationServiceSelectorSection
      grouped
      available
      provider="none"
      showAccountProvider={false}
      offlineFallback="dictionary"
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByRole("option", { name: "关闭" })).toBeNull();
  expect(screen.queryByRole("option", { name: "离线翻译（不联网）" })).toBeNull();
  const none = screen.getByRole("option", { name: "不使用在线服务" }) as HTMLOptionElement;
  expect(none.value).toBe("none");
  expect(screen.getByText(/离线词库查得到的释义/)).toBeTruthy();
  expect(screen.queryByText(/macOS 26/)).toBeNull();
});
