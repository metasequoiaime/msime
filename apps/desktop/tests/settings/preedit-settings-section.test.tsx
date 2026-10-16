// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PreeditSettingsSection, type PreeditSettingsPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: PreeditSettingsPreferences = {
  shuangpin_preedit_uses_raw: true,
  tsf_preedit_style: "raw",
  candidate_preedit_style: "pinyin",
};

test("missing preferences preserve the preedit defaults and each selector's ordered choices", () => {
  render(
    <PreeditSettingsSection
      preferences={{}}
      mobile={false}
      showShuangpinPreedit
      inlinePreeditBusy={false}
      onChange={vi.fn()}
    />,
  );

  const choices = (label: string) => {
    const select = screen.getByLabelText(label) as HTMLSelectElement;
    return {
      value: select.value,
      options: Array.from(select.options, (option) => [option.value, option.textContent]),
    };
  };
  expect(choices("候选窗口预编辑")).toEqual({
    value: "pinyin",
    options: [
      ["pinyin", "拼音分词"],
      ["empty", "不显示"],
    ],
  });
  expect(choices("双拼预编辑")).toEqual({
    value: "raw",
    options: [
      ["raw", "原始按键"],
      ["pinyin", "拼音分词"],
    ],
  });
  expect(choices("行内预编辑")).toEqual({
    value: "raw",
    options: [
      ["raw", "原始按键"],
      ["pinyin", "拼音分词"],
      ["empty", "不显示"],
    ],
  });
});

test("desktop preedit selectors report each changed preference", () => {
  const onChange = vi.fn();
  render(
    <PreeditSettingsSection
      preferences={preferences}
      mobile={false}
      showShuangpinPreedit
      onChange={onChange}
    />,
  );

  // 候选窗口自己的预编辑排在前面，写进应用里的预编辑排在最后。
  expect(
    [...document.querySelectorAll("[data-row-title]")].map((title) => title.textContent),
  ).toEqual(["候选窗口预编辑", "双拼预编辑", "行内预编辑"]);
  fireEvent.change(screen.getByLabelText("双拼预编辑"), { target: { value: "pinyin" } });
  expect(onChange).toHaveBeenCalledWith({ shuangpin_preedit_uses_raw: false });
  fireEvent.change(screen.getByLabelText("行内预编辑"), { target: { value: "empty" } });
  expect(onChange).toHaveBeenCalledWith({ tsf_preedit_style: "empty" });
  fireEvent.change(screen.getByLabelText("候选窗口预编辑"), { target: { value: "empty" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_preedit_style: "empty" });
});

// 触屏键盘固定把拼写写进输入框、候选栏不画它，所以只剩双拼预编辑。
test("touch preedit offers only the shuangpin choice", () => {
  render(
    <PreeditSettingsSection
      preferences={preferences}
      mobile
      showShuangpinPreedit
      onChange={vi.fn()}
    />,
  );

  expect(
    [...document.querySelectorAll("[data-row-title]")].map((title) => title.textContent),
  ).toEqual(["双拼预编辑"]);
  expect(screen.queryByLabelText("行内预编辑")).toBeNull();
  expect(screen.queryByLabelText("候选栏预编辑")).toBeNull();
});
