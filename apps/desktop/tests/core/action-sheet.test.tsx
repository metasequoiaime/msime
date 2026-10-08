// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState, type ReactNode } from "react";
import { ActionSheet, type SheetOption } from "../../../../packages/ui/src/core/action-sheet";
import {
  GroupList,
  NavGroup,
  NavRow,
  Switch,
} from "../../../../packages/ui/src/core/platform-controls";
import { SegmentedRow } from "../../../../packages/ui/src/settings/segmented-row";
import { SelectRow } from "../../../../packages/ui/src/settings/select-row";
import { dictionaryFormatOptions } from "../../../../packages/ui/src/dictionary/dictionary-format-options";
import { FeedbackReportFields } from "../../../../packages/ui/src/settings/feedback-report-fields";
import { InputSchemeDetailsSection } from "../../../../packages/ui/src/settings/input-scheme-details-section";
import {
  SettingsFormContext,
  type SettingsFormModel,
} from "../../../../packages/ui/src/settings/settings-form-context";

afterEach(cleanup);

const themes: SheetOption[] = [
  { value: "siji", label: "水杉四季（自动）", selected: true },
  { value: "chunya", label: "春芽" },
  { value: "dongxue", label: "冬雪", disabled: true },
];

function Sheet({
  options = themes,
  onSelect = vi.fn(),
  onClose,
}: {
  options?: SheetOption[];
  onSelect?: (value: string) => void;
  onClose?: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div data-platform="harmony">
      <button type="button" onClick={() => setOpen(true)}>
        应用主题
      </button>
      <ActionSheet
        open={open}
        title="应用主题"
        subtitle="四季会随季节自动更换配色"
        options={options}
        onSelect={onSelect}
        onClose={() => {
          setOpen(false);
          onClose?.();
        }}
      />
    </div>
  );
}

test("a sheet opens as a modal dialog in place under the platform root, with the current option marked and focused first", () => {
  const { container } = render(<Sheet />);
  expect(screen.queryByRole("dialog")).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "应用主题" }));
  const dialog = screen.getByRole("dialog", { name: "应用主题" });
  expect(dialog.getAttribute("aria-modal")).toBe("true");
  // 就地渲染在 `[data-platform]` 元素之下，所以平台和季节 token 能作用到它。
  expect(container.querySelector("[data-platform]")?.contains(dialog)).toBe(true);
  expect(within(dialog).getByText("四季会随季节自动更换配色")).toBeTruthy();

  const current = within(dialog).getByRole("button", { name: "水杉四季（自动）" });
  expect(current.getAttribute("aria-current")).toBe("true");
  expect(current.className).toContain("font-semibold");
  expect(current.querySelector("svg")).toBeTruthy();
  expect(document.activeElement).toBe(current);
  expect((within(dialog).getByRole("button", { name: "冬雪" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
});

test("choosing an option closes the sheet before reporting the choice", () => {
  const events: string[] = [];
  render(
    <Sheet
      onSelect={(value) => events.push(`select ${value}`)}
      onClose={() => events.push("close")}
    />,
  );
  const opener = screen.getByRole("button", { name: "应用主题" });
  opener.focus();
  fireEvent.click(opener);
  fireEvent.click(screen.getByRole("button", { name: "春芽" }));

  expect(events).toEqual(["close", "select chunya"]);
  expect(screen.queryByRole("dialog")).toBeNull();
  // 焦点回到打开它的控件。
  expect(document.activeElement).toBe(opener);
});

test("a destructive option is drawn in red", () => {
  render(
    <Sheet
      options={[
        { value: "edit", label: "编辑" },
        { value: "remove", label: "移除粤语", destructive: true },
      ]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "应用主题" }));
  expect(screen.getByRole("button", { name: "移除粤语" }).className).toContain("#FF3B30");
  expect(screen.getByRole("button", { name: "编辑" }).className).not.toContain("#FF3B30");
});

test("a submenu swaps the sheet to its level, goes back, and reports the nested choice", () => {
  const onSelect = vi.fn();
  render(
    <Sheet
      onSelect={onSelect}
      options={[
        { value: "quanpin", label: "全拼" },
        {
          value: "shuangpin",
          label: "双拼",
          submenu: {
            title: "双拼方案",
            options: [
              { value: "xiaohe", label: "小鹤双拼", selected: true },
              { value: "ziranma", label: "自然码" },
            ],
          },
        },
      ]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "应用主题" }));
  fireEvent.click(screen.getByRole("button", { name: /双拼/ }));

  const dialog = screen.getByRole("dialog", { name: "双拼方案" });
  expect(within(dialog).queryByRole("button", { name: "全拼" })).toBeNull();
  expect(document.activeElement).toBe(within(dialog).getByRole("button", { name: "小鹤双拼" }));

  fireEvent.click(within(dialog).getByRole("button", { name: "返回上一级" }));
  expect(screen.getByRole("dialog", { name: "应用主题" })).toBeTruthy();
  expect(onSelect).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: /双拼/ }));
  // 在子菜单里按 Escape 是返回上一级而不是关闭。
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(screen.getByRole("dialog", { name: "应用主题" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: /双拼/ }));
  fireEvent.click(screen.getByRole("button", { name: "自然码" }));
  expect(onSelect).toHaveBeenCalledWith("ziranma");
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("取消, the scrim and Escape close the sheet without a choice", () => {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  const { container } = render(<Sheet onSelect={onSelect} onClose={onClose} />);
  const open = () => fireEvent.click(screen.getByRole("button", { name: "应用主题" }));

  open();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("dialog")).toBeNull();

  open();
  const scrim = container.querySelector('[data-platform] div[aria-hidden="true"]');
  expect(scrim).toBeTruthy();
  fireEvent.click(scrim!);
  expect(screen.queryByRole("dialog")).toBeNull();

  open();
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();

  expect(onClose).toHaveBeenCalledTimes(3);
  expect(onSelect).not.toHaveBeenCalled();
});

function OnHarmonyPhone({ children }: { children: ReactNode }) {
  return (
    <SettingsFormContext.Provider value={{ settingsPlatform: "harmony" } as SettingsFormModel}>
      <div data-platform="harmony">{children}</div>
    </SettingsFormContext.Provider>
  );
}

function SchemeRow({ onChange }: { onChange: (value: string) => void }) {
  const [scheme, setScheme] = useState("ziranma");
  return (
    <GroupList>
      <SelectRow
        title="全拼辅助码方案"
        value={scheme}
        onChange={(event) => {
          setScheme(event.target.value);
          onChange(event.target.value);
        }}
      >
        <option value="ziranma">自然码</option>
        <option value="xiaohe">小鹤</option>
      </SelectRow>
    </GroupList>
  );
}

test("on the HarmonyOS phone a select row is one button that picks through the sheet", () => {
  const onChange = vi.fn();
  render(
    <OnHarmonyPhone>
      <SchemeRow onChange={onChange} />
    </OnHarmonyPhone>,
  );
  const row = screen.getByRole("button", { name: /^全拼辅助码方案/ });
  expect(row.getAttribute("aria-haspopup")).toBe("dialog");
  expect(within(row).getByText("自然码")).toBeTruthy();

  fireEvent.click(row);
  const dialog = screen.getByRole("dialog", { name: "全拼辅助码方案" });
  expect(within(dialog).getByRole("button", { name: "自然码" }).getAttribute("aria-current")).toBe(
    "true",
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "小鹤" }));

  // 选择以原生 select 的 change 事件传给调用方。
  expect(onChange).toHaveBeenCalledWith("xiaohe");
  expect(within(row).getByText("小鹤")).toBeTruthy();
  // 原生 select 仍留在页面中，处于 inert 状态并保存相同的值；行标题是它的标签，且只标注它。
  const select = screen.getByLabelText("全拼辅助码方案") as HTMLSelectElement;
  expect(select.tagName).toBe("SELECT");
  expect(select.value).toBe("xiaohe");
  expect(select.closest("[inert]")).toBeTruthy();
});

test("on the HarmonyOS phone the shared option lists fill the sheet", () => {
  const sheetChoices = (title: string) => {
    fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${title}`) }));
    const dialog = screen.getByRole("dialog", { name: title });
    const labels = within(dialog)
      .getAllByRole("button")
      .map((button) => button.textContent);
    fireEvent.keyDown(dialog, { key: "Escape" });
    return labels;
  };
  render(
    <OnHarmonyPhone>
      <GroupList>
        <InputSchemeDetailsSection
          scheme="quanpin"
          shuangpinProfile="xiaohe"
          wubiProfile="wubi98"
          macos={false}
          hasTouchKeyboardSchemes
          touchKeyboardHasWubi
          onShuangpinProfileChange={vi.fn()}
          onWubiProfileChange={vi.fn()}
        />
        <FeedbackReportFields
          kind="功能建议"
          detail=""
          onKindChange={vi.fn()}
          onDetailChange={vi.fn()}
        />
        <SelectRow title="文件格式" value="rime" onChange={vi.fn()}>
          {dictionaryFormatOptions({ pinyin: true, rime: true })}
        </SelectRow>
      </GroupList>
    </OnHarmonyPhone>,
  );
  // 鸿蒙手机启用了五笔键盘时显示「五笔方案」行。行上显示存储的值，而不是空白或回退到第一项。
  expect(screen.getByRole("button", { name: /^五笔方案/ }).textContent).toContain("98 五笔");
  expect(screen.getByRole("button", { name: /^类型/ }).textContent).toContain("功能建议");
  expect(screen.getByRole("button", { name: /^文件格式/ }).textContent).toContain("Rime");
  expect(sheetChoices("五笔方案")).toEqual(expect.arrayContaining(["86 五笔", "98 五笔"]));
  expect(sheetChoices("类型")).toEqual(
    expect.arrayContaining(["功能异常", "候选词不对", "功能建议", "其他"]),
  );
  expect(sheetChoices("文件格式")).toEqual(
    expect.arrayContaining(["编码在前（Windows TSV）", "汉字自动注音（仅导入）"]),
  );
});

test("elsewhere a select row keeps the native select", () => {
  render(<SchemeRow onChange={vi.fn()} />);
  expect(screen.getByRole("combobox", { name: "全拼辅助码方案" }).closest("[inert]")).toBeNull();
  expect(screen.queryByRole("button", { name: /全拼辅助码方案/ })).toBeNull();
});

test("on the HarmonyOS phone a segmented row picks through the sheet too", () => {
  function Mode() {
    const [mode, setMode] = useState<"light" | "dark" | "system">("system");
    return (
      <SegmentedRow
        title="外观"
        value={mode}
        options={[
          { value: "light", label: "浅色" },
          { value: "dark", label: "深色" },
          { value: "system", label: <span>跟随系统</span> },
        ]}
        onChange={setMode}
      />
    );
  }
  render(
    <OnHarmonyPhone>
      <Mode />
    </OnHarmonyPhone>,
  );
  const row = screen.getByRole("button", { name: /^外观/ });
  expect(within(row).getByText("跟随系统")).toBeTruthy();
  fireEvent.click(row);
  fireEvent.click(screen.getByRole("button", { name: "深色" }));
  expect(within(row).getByText("深色")).toBeTruthy();
  expect((screen.getByRole("radio", { name: "深色" }) as HTMLInputElement).checked).toBe(true);
});

test("a 我的 row names its value and opens a sheet; a row holding a switch names the switch", () => {
  function Rows() {
    const [open, setOpen] = useState(false);
    const [haptics, setHaptics] = useState(false);
    return (
      <NavGroup>
        <NavRow variant="me" title="应用主题" value="四季 · 秋" onClick={() => setOpen(true)} />
        <NavRow
          variant="me"
          title="按键振动"
          description="轻点按键时振动"
          trailing={<Switch checked={haptics} onChange={setHaptics} />}
        />
        <ActionSheet
          open={open}
          title="应用主题"
          options={themes}
          onSelect={vi.fn()}
          onClose={() => setOpen(false)}
        />
      </NavGroup>
    );
  }
  render(<Rows />);
  const theme = screen.getByRole("button", { name: "应用主题" });
  expect(theme.getAttribute("aria-describedby")).toBe(screen.getByText("四季 · 秋").id);
  fireEvent.click(theme);
  expect(screen.getByRole("dialog", { name: "应用主题" })).toBeTruthy();

  const toggle = screen.getByRole("switch", { name: "按键振动" });
  expect(toggle.getAttribute("aria-describedby")).toBe(screen.getByText("轻点按键时振动").id);
  expect(screen.queryByRole("button", { name: "按键振动" })).toBeNull();
});
