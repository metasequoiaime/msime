// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { HelpcodeSettingsPage } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("helper-code settings expose independent scheme updates", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
        shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
      }}
      mobile={false}
      showShiftEntry
      onChange={onChange}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "全拼辅助码" }));

  expect(onChange).toHaveBeenCalledWith({
    quanpin_helpcode: { enabled: false, schema: "ziranma", show_in_candidate_window: false },
  });
});

test("helper-code settings include discovered custom schemas", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
        shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
      }}
      mobile={false}
      showShiftEntry
      customSchemas={[
        {
          schema: "custom/synthetic",
          file_stem: "synthetic",
          name: "合成辅助码",
          name_en: "Synthetic",
        },
        { schema: "custom/english", file_stem: "english", name: "", name_en: "English" },
        { schema: "custom/fallback", file_stem: "fallback", name: "", name_en: "" },
      ]}
      onChange={onChange}
    />,
  );

  expect(screen.getAllByRole("option", { name: "合成辅助码 (Synthetic)" })).toHaveLength(2);
  expect(screen.getAllByRole("option", { name: "English" })).toHaveLength(2);
  expect(screen.getAllByRole("option", { name: "fallback" })).toHaveLength(2);

  fireEvent.change(screen.getByRole("combobox", { name: "全拼辅助码方案" }), {
    target: { value: "custom/synthetic" },
  });
  expect(onChange).toHaveBeenCalledWith({
    quanpin_helpcode: {
      enabled: true,
      schema: "custom/synthetic",
      show_in_candidate_window: false,
    },
  });
});

test("helper-code settings retain an unavailable custom selection", () => {
  const onChange = vi.fn();

  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: {
          enabled: true,
          schema: "custom/no-longer-installed",
          show_in_candidate_window: false,
        },
      }}
      mobile={false}
      showShiftEntry={false}
      onChange={onChange}
    />,
  );

  expect(screen.getByRole("option", { name: "custom/no-longer-installed" })).toBeTruthy();
  expect(
    (screen.getByRole("combobox", { name: "全拼辅助码方案" }) as HTMLSelectElement).value,
  ).toBe("custom/no-longer-installed");
});

test("helper-code settings offer installed helpcode packs per scheme", () => {
  const onChange = vi.fn();
  const helpcode = {
    quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
    shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
  } as const;
  const view = render(
    <HelpcodeSettingsPage
      value={helpcode}
      mobile={false}
      showShiftEntry
      packs={[{ id: "radicals", name: "部首码" }]}
      onChange={onChange}
    />,
  );
  expect(screen.getAllByRole("option", { name: "部首码（插件）" })).toHaveLength(2);
  fireEvent.change(screen.getByRole("combobox", { name: "全拼辅助码方案" }), {
    target: { value: "pack:radicals" },
  });
  expect(onChange).toHaveBeenLastCalledWith({
    plugins: expect.objectContaining({
      helpcode_pack_quanpin: "radicals",
      helpcode_pack_shuangpin: "",
    }),
  });

  // 选中的包显示在下拉框里；不在本机时标为未找到；选回内置方案时清空它。
  view.rerender(
    <HelpcodeSettingsPage
      value={
        {
          ...helpcode,
          plugins: { helpcode_pack_quanpin: "radicals", helpcode_pack_shuangpin: "gone" },
        } as never
      }
      mobile={false}
      showShiftEntry
      packs={[{ id: "radicals", name: "部首码" }]}
      onChange={onChange}
    />,
  );
  const quanpin = screen.getByRole("combobox", { name: "全拼辅助码方案" }) as HTMLSelectElement;
  expect(quanpin.value).toBe("pack:radicals");
  expect(screen.getByRole("option", { name: "gone（插件，未找到）" })).toBeTruthy();
  fireEvent.change(quanpin, { target: { value: "xiaohe" } });
  expect(onChange).toHaveBeenLastCalledWith({
    quanpin_helpcode: { enabled: true, schema: "xiaohe", show_in_candidate_window: false },
    plugins: expect.objectContaining({
      helpcode_pack_quanpin: "",
      helpcode_pack_shuangpin: "gone",
    }),
  });
});

test("desktop shows the active family's three rows and folds the other family", () => {
  render(
    <HelpcodeSettingsPage
      value={{}}
      mobile={false}
      showShiftEntry={false}
      activeFamily="shuangpin_helpcode"
      onChange={vi.fn()}
    />,
  );
  const fold = document.querySelector("details");
  expect(fold).not.toBeNull();
  for (const name of ["双拼辅助码", "在候选窗口中显示双拼辅助码"]) {
    expect(fold?.contains(screen.getByRole("switch", { name }))).toBe(false);
  }
  expect(fold?.contains(screen.getByRole("combobox", { name: "双拼辅助码方案" }))).toBe(false);
  // 另一族对当前方案不起作用，收进「更多选项」但仍可改。
  for (const name of ["全拼辅助码", "在候选窗口中显示全拼辅助码"]) {
    expect(fold?.contains(screen.getByRole("switch", { name }))).toBe(true);
  }
  expect(fold?.contains(screen.getByRole("combobox", { name: "全拼辅助码方案" }))).toBe(true);
});

test("没有导入能力的宿主不显示空操作入口", () => {
  render(<HelpcodeSettingsPage value={{}} mobile showShiftEntry={false} onChange={vi.fn()} />);
  expect(screen.queryByText("导入辅助码表")).toBeNull();
});

test("辅助码前导键按全拼和双拼独立保存，并保留当前方案", () => {
  const onChange = vi.fn();
  render(
    <HelpcodeSettingsPage
      value={{
        quanpin_helpcode: { enabled: true, schema: "xiaohe", trigger_key: ";" },
        shuangpin_helpcode: { enabled: true, schema: "lantian", trigger_key: "/" },
      }}
      mobile={false}
      showShiftEntry={false}
      showTriggerKey
      onChange={onChange}
    />,
  );
  const quanpin = screen.getByRole("combobox", { name: "全拼辅助码触发键" }) as HTMLSelectElement;
  const shuangpin = screen.getByRole("combobox", { name: "双拼辅助码触发键" }) as HTMLSelectElement;
  expect(quanpin.value).toBe(";");
  expect(shuangpin.value).toBe("/");
  fireEvent.change(quanpin, { target: { value: "`" } });
  expect(onChange).toHaveBeenLastCalledWith({
    quanpin_helpcode: {
      enabled: true,
      schema: "xiaohe",
      trigger_key: "`",
      show_in_candidate_window: false,
    },
  });
  fireEvent.change(shuangpin, { target: { value: "" } });
  expect(onChange).toHaveBeenLastCalledWith({
    shuangpin_helpcode: {
      enabled: true,
      schema: "lantian",
      trigger_key: "",
      show_in_candidate_window: true,
      modes: {
        manual_tab: false,
        direct: true,
        trigger_backtick: false,
        trigger_semicolon: true,
        trigger_uppercase: false,
        mid_sentence: false,
        mid_sentence_backtick: true,
        mid_sentence_semicolon: false,
        mid_sentence_uppercase: false,
        direct_helpcode: false,
        direct_helpcode_slash: true,
        direct_helpcode_uppercase: false,
      },
    },
  });
});

test("未报告前导键能力的宿主不显示配置", () => {
  render(
    <HelpcodeSettingsPage value={{}} mobile={false} showShiftEntry={false} onChange={vi.fn()} />,
  );
  expect(screen.queryByRole("combobox", { name: "全拼辅助码触发键" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "双拼辅助码触发键" })).toBeNull();
});

test("Tab 手动辅助码开关保留其它触发方式", () => {
  const onChange = vi.fn();
  const value = { shuangpin_helpcode: { enabled: true, schema: "lantian" as const } };
  const { rerender } = render(
    <HelpcodeSettingsPage
      value={value}
      mobile={false}
      showShiftEntry
      showManualTab
      onChange={onChange}
    />,
  );
  fireEvent.click(screen.getByRole("switch", { name: "双拼手动辅助码（Tab）" }));
  const changed = onChange.mock.calls[0][0];
  expect(changed.shuangpin_helpcode.modes).toMatchObject({
    manual_tab: true,
    direct: true,
    trigger_semicolon: true,
  });
  rerender(
    <HelpcodeSettingsPage
      value={changed}
      mobile={false}
      showShiftEntry
      showManualTab
      onChange={onChange}
    />,
  );
  expect(
    (screen.getByRole("switch", { name: "双拼末尾直接辅助码" }) as HTMLButtonElement).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("switch", { name: "双拼手动辅助码（Tab）" }));
  expect(onChange.mock.calls[1][0].shuangpin_helpcode.modes).toMatchObject({
    manual_tab: false,
    direct: true,
    trigger_semicolon: true,
  });
});

test("不支持 Tab 的宿主不提供启用入口", () => {
  render(<HelpcodeSettingsPage value={{}} mobile={false} showShiftEntry onChange={vi.fn()} />);
  expect(screen.queryByRole("switch", { name: "双拼手动辅助码（Tab）" })).toBeNull();
});
