// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import styles from "../../../../packages/ui/src/styles.css?raw";
import controlStyles from "../../../../packages/ui/src/core/platform-controls-style.ts?raw";
import {
  Checks,
  GroupList,
  NavItem,
  Row,
  Segmented,
  Select,
  Slider,
  Switch,
} from "../../../../packages/ui/src/core/platform-controls";
import {
  platformCssVariables,
  platformTokens,
} from "../../../../packages/ui/src/theme/platform-tokens";
import { utilityCss } from "../support/utility-css";

afterEach(cleanup);

function ToggleRow({ onChange }: { onChange: (checked: boolean) => void }) {
  const [on, setOn] = useState(false);
  return (
    <GroupList title="输入">
      <Row title="模糊音" description="把 zh 和 z 当作同一个声母">
        <Switch
          checked={on}
          onChange={(value) => {
            setOn(value);
            onChange(value);
          }}
        />
      </Row>
    </GroupList>
  );
}

test("a group is a region named by its title, and a row names and describes its control", () => {
  const onChange = vi.fn();
  render(<ToggleRow onChange={onChange} />);
  const group = screen.getByRole("region", { name: "输入" });
  const toggle = within(group).getByRole("switch", { name: "模糊音" });
  expect(toggle.getAttribute("aria-describedby")).toBe(
    screen.getByText("把 zh 和 z 当作同一个声母").id,
  );
  expect((toggle as HTMLInputElement).checked).toBe(false);

  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(true);
  expect((screen.getByRole("switch", { name: "模糊音" }) as HTMLInputElement).checked).toBe(true);
});

test("a group without a title is not announced as a landmark", () => {
  render(
    <GroupList>
      <Row title="候选数量" />
    </GroupList>,
  );
  expect(screen.queryByRole("region")).toBeNull();
  expect(screen.getByText("候选数量")).toBeTruthy();
});

test("a hidden row stays on the page with its name but leaves the accessibility tree", () => {
  render(
    <GroupList title="方案">
      <Row title="双拼方案" hidden>
        <Select value="xiaohe" onChange={vi.fn()}>
          <option value="xiaohe">小鹤双拼</option>
        </Select>
      </Row>
      <Row title="五笔方案">
        <Select value="wubi86" onChange={vi.fn()}>
          <option value="wubi86">86 五笔</option>
        </Select>
      </Row>
    </GroupList>,
  );
  expect(screen.queryByRole("combobox", { name: "双拼方案" })).toBeNull();
  expect(screen.getByRole("combobox", { name: "五笔方案" })).toBeTruthy();
  // Whatever reads a page's settings back still finds the hidden one, and the group's name is marked apart from the rows'.
  expect(
    [...document.querySelectorAll("[data-row-title]")].map((node) => node.textContent),
  ).toEqual(["双拼方案", "五笔方案"]);
  expect(document.querySelector("[data-group-title]")?.textContent).toBe("方案");
});

test("the switch keeps its own name when given one, and the Windows 开/关 text stays out of it", () => {
  render(
    <Row title="行标题">
      <Switch checked onChange={() => {}} aria-label="自定义名称" />
    </Row>,
  );
  const toggle = screen.getByRole("switch", { name: "自定义名称" });
  expect((toggle as HTMLInputElement).checked).toBe(true);
  const state = screen.getByText("开");
  expect(state.getAttribute("aria-hidden")).toBe("true");
});

test("a disabled switch cannot be turned on", () => {
  const onChange = vi.fn();
  render(<Switch checked={false} onChange={onChange} disabled aria-label="云同步" />);
  const toggle = screen.getByRole("switch", { name: "云同步" }) as HTMLInputElement;
  expect(toggle.disabled).toBe(true);
});

test("the segmented control is a radio group with one checked option", () => {
  function Mode() {
    const [mode, setMode] = useState<"light" | "dark" | "system">("system");
    return (
      <Row title="外观">
        <Segmented
          options={[
            { value: "light", label: "浅色" },
            { value: "dark", label: "深色" },
            { value: "system", label: "跟随系统" },
          ]}
          value={mode}
          onChange={setMode}
        />
      </Row>
    );
  }
  render(<Mode />);
  const group = screen.getByRole("radiogroup", { name: "外观" });
  const radios = within(group).getAllByRole("radio") as HTMLInputElement[];
  expect(radios.map((radio) => radio.checked)).toEqual([false, false, true]);
  expect(new Set(radios.map((radio) => radio.name)).size).toBe(1);

  fireEvent.click(within(group).getByRole("radio", { name: "深色" }));
  expect(
    (within(group).getAllByRole("radio") as HTMLInputElement[]).map((radio) => radio.checked),
  ).toEqual([false, true, false]);
});

test("a disabled segment is skipped", () => {
  const onChange = vi.fn();
  render(
    <Segmented
      aria-label="布局"
      options={[
        { value: "h", label: "横排" },
        { value: "v", label: "竖排", disabled: true },
      ]}
      value="h"
      onChange={onChange}
    />,
  );
  expect((screen.getByRole("radio", { name: "竖排" }) as HTMLInputElement).disabled).toBe(true);
});

test("the select stays a native select named by its row", () => {
  const onChange = vi.fn();
  render(
    <Row title="每页候选" description="3 到 9 个">
      <Select value="5" onChange={(event) => onChange(event.currentTarget.value)}>
        <option value="5">5</option>
        <option value="7">7</option>
      </Select>
    </Row>,
  );
  const select = screen.getByRole("combobox", { name: "每页候选" });
  expect(select.tagName).toBe("SELECT");
  expect(select.getAttribute("aria-describedby")).toBe(screen.getByText("3 到 9 个").id);
  fireEvent.change(select, { target: { value: "7" } });
  expect(onChange).toHaveBeenCalledWith("7");
});

test("the slider is a range input that reports its value and fills its track", () => {
  const onChange = vi.fn();
  render(
    <Row title="字号">
      <Slider value={15} min={10} max={20} valueText="15 号" onChange={onChange} />
    </Row>,
  );
  const slider = screen.getByRole("slider", { name: "字号" }) as HTMLInputElement;
  expect(slider.type).toBe("range");
  expect(slider.value).toBe("15");
  expect(slider.getAttribute("aria-valuetext")).toBe("15 号");
  expect(slider.style.getPropertyValue("--slider-fill")).toBe("50%");
  fireEvent.change(slider, { target: { value: "18" } });
  expect(onChange).toHaveBeenCalledWith(18);
});

test("checks are a group of checkboxes named by their legend, with a mixed state", () => {
  const onChange = vi.fn();
  render(
    <Checks
      legend="切换中英文"
      items={[
        { value: "shift", label: "Shift", checked: true },
        { value: "ctrl", label: "Ctrl", checked: false },
        { value: "all", label: "全部", checked: "mixed" },
      ]}
      onChange={onChange}
    />,
  );
  const group = screen.getByRole("group", { name: "切换中英文" });
  const [shift, ctrl, all] = within(group).getAllByRole("checkbox") as HTMLInputElement[];
  expect([shift.checked, ctrl.checked, all.checked]).toEqual([true, false, false]);
  expect([shift.indeterminate, ctrl.indeterminate, all.indeterminate]).toEqual([
    false,
    false,
    true,
  ]);
  fireEvent.click(within(group).getByRole("checkbox", { name: "Ctrl" }));
  expect(onChange).toHaveBeenCalledWith("ctrl", true);
});

test("a check group's description describes the group, not its name", () => {
  render(
    <Checks
      legend="工具栏组件"
      description="勾选要显示在悬浮工具栏中的功能"
      items={[{ value: "emoji", label: "表情与符号", checked: true }]}
      onChange={vi.fn()}
    />,
  );
  const group = screen.getByRole("group", { name: "工具栏组件" });
  expect(group.getAttribute("aria-describedby")).toBeTruthy();
  expect(document.getElementById(group.getAttribute("aria-describedby")!)?.textContent).toBe(
    "勾选要显示在悬浮工具栏中的功能",
  );
});

test("only the selected navigation item is the current page", () => {
  const onSelect = vi.fn();
  render(
    <nav aria-label="设置分类">
      <NavItem label="外观" selected onSelect={() => {}} />
      <NavItem label="输入" selected={false} onSelect={onSelect} />
    </nav>,
  );
  const nav = screen.getByRole("navigation", { name: "设置分类" });
  expect(within(nav).getByRole("button", { name: "外观" }).getAttribute("aria-current")).toBe(
    "page",
  );
  const input = within(nav).getByRole("button", { name: "输入" });
  expect(input.getAttribute("aria-current")).toBeNull();
  fireEvent.click(input);
  expect(onSelect).toHaveBeenCalledOnce();
});

test("every platform token the controls read is one the platform layer defines", () => {
  const defined = new Set(Object.keys(platformCssVariables(platformTokens.win.light)));
  const sources = [
    controlStyles,
    utilityCss("platform-switch"),
    utilityCss("platform-check"),
    utilityCss("platform-slider"),
  ];
  const read = new Set(
    sources.flatMap((source) =>
      [...source.matchAll(/var\((--p-[a-z0-9-]+)/g)].map((match) => match[1]),
    ),
  );
  expect(read.size).toBeGreaterThan(40);
  expect([...read].filter((name) => !defined.has(name))).toEqual([]);
  expect(styles).toContain("@utility platform-switch");
});

test("the controls change shape per platform through the structural tokens", () => {
  const variables = (platform: keyof typeof platformTokens) =>
    platformCssVariables(platformTokens[platform].light);
  expect(variables("win")["--p-sw-label"]).toBe("inline");
  expect(variables("mac")["--p-sw-label"]).toBe("none");
  expect(variables("win")["--p-slider-appearance"]).toBe("auto");
  expect(variables("android")["--p-slider-appearance"]).toBe("none");
  expect(variables("win")["--p-nav-pill"]).toBe("3px");
  expect(variables("hm2")["--p-nav-pill"]).toBe("0");
  expect(variables("win")["--p-row-icon"]).toBe("flex");
  expect(variables("ios")["--p-row-icon"]).toBe("none");
  expect(variables("win")["--p-seg-indicator"]).toBe("16px");
  expect(variables("linux")["--p-seg-indicator"]).toBe("0");
  expect(platformTokens.win.light.group.style).toBe("card");
  expect(platformTokens.mac.light.group.style).toBe("inset");
  expect(platformTokens.android.light.group.style).toBe("separated");
  expect(variables("win")["--p-row-divider"]).toBe("transparent");
  expect(variables("android")["--p-row-divider"]).toBe("transparent");
  expect(variables("mac")["--p-row-divider"]).toBe(platformTokens.mac.light.hair);
});
