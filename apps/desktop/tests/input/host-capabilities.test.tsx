// @vitest-environment jsdom
import { selectSettingsTab, settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type SettingsClient, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

// The platform-dependent fields follow what `client-core::host_surface` reports for that platform; a test overrides the one it is about.
function capabilities(overrides: Partial<HostCapabilities> = {}): HostCapabilities {
  const platform = overrides.platform ?? "windows";
  const mobile = platform === "android" || platform === "ios" || platform === "harmony";
  const touchStrip = platform === "android" || platform === "ios";
  return {
    platform,
    restart_input_method: true,
    panel_windows: true,
    ime_mode_scope: false,
    typing_statistics: true,
    fuzzy_pinyin: false,
    system_fonts: true,
    window_chrome: true,
    floating_toolbar: true,
    floating_toolbar_appearance: true,
    floating_toolbar_components: true,
    mode_switch_shortcuts: false,
    panel_shortcuts: false,
    voice_capture_devices: false,
    candidate_font_controls: true,
    candidate_row_colors: true,
    candidate_selection_appearance: true,
    candidate_follow_cursor: true,
    input_mode_hud: false,
    input_mode_status_item: false,
    candidate_english_font: false,
    english_suggestions: false,
    shuangpin_preedit: false,
    voice_commit_mode: false,
    mobile_settings: mobile,
    vocabulary_review: false,
    floating_toolbar_handwriting: false,
    floating_toolbar_voice: false,
    floating_toolbar_input_scheme: false,
    number_row_selection: false,
    candidate_preedit_font: true,
    candidate_page_number: false,
    // 桌面宿主的编号色和边框色跟随主题，各界面跟随颜色模式；只有 Android 和 iOS 的键盘照旧读这几项。
    candidate_border_color: touchStrip,
    candidate_number_color: touchStrip,
    surface_themes_follow: !touchStrip,
    candidate_window_scale: touchStrip,
    candidate_window_opacity: false,
    candidate_corner_radius: false,
    candidate_padding: false,
    helpcode_shift_entry: mobile,
    skin_directory_import: false,
    touch_toolbar_components: false,
    maintenance_shortcuts: false,
    fullwidth_chord: platform === "macos",
    voice_provider_settings: platform !== "android",
    voice_stream_preedit: platform !== "android",
    character_width: !mobile,
    ai_provider_credentials: platform === "linux",
    key_sound: false,
    plugin_triggers: false,
    music: false,
    typing_effects: false,
    wordbook_packs: false,
    symbol_set_packs: false,
    input_schemes: ["quanpin", "shuangpin", "wubi", "japanese", "korean"],
    ...overrides,
  };
}

function mount(client: Partial<SettingsClient>) {
  return render(<SettingsPage client={{ load: async () => initial, save: vi.fn(), ...client }} />);
}

test("host capabilities decide platform-specific settings instead of the user agent", async () => {
  // A Windows host that tracks session-wide mode gets the control; the gate is
  // the capability, not the platform name.
  mount({ host: capabilities({ platform: "windows", ime_mode_scope: true }) });
  await settingsFormReady();
  expect(screen.getByLabelText("中英文状态")).toBeTruthy();
});

test("a macOS host receives its native mode scope control", async () => {
  mount({ host: capabilities({ platform: "macos", ime_mode_scope: true }) });
  await settingsFormReady();
  expect(screen.getByText("按应用分别记忆输入状态，或让所有输入上下文保持同一状态")).toBeTruthy();
  expect(screen.getByLabelText("中英文状态")).toBeTruthy();
});

test("a touch host that can name the editor's application gets the mode scope control", async () => {
  // HarmonyOS reads the bundle name off the editor attribute, so a per-application map is real
  // there; the gate stays the capability rather than a list of desktop platform names.
  mount({ host: capabilities({ platform: "harmony", ime_mode_scope: true }) });
  await screen.findByLabelText("中英文状态");
  cleanup();
  mount({ host: capabilities({ platform: "android", ime_mode_scope: false }) });
  await settingsFormReady();
  expect(screen.queryByLabelText("中英文状态")).toBeNull();
});

test("a host without mode scope support does not receive the control", async () => {
  mount({ host: capabilities({ platform: "windows" }) });
  await settingsFormReady();
  expect(screen.queryByLabelText("中英文状态")).toBeNull();
});

test("without a host the mode scope control is hidden", async () => {
  mount({});
  await settingsFormReady();
  expect(screen.queryByLabelText("中英文状态")).toBeNull();
});

test("typing statistics follow the injected client on any platform", async () => {
  const statistics = {
    load: vi.fn().mockResolvedValue({ enabled: false, statistics: null }),
    setEnabled: vi.fn().mockResolvedValue(undefined),
    reset: vi.fn().mockResolvedValue(undefined),
  };
  // Previously this category was reachable only when the user agent matched Android.
  mount({ host: capabilities({ platform: "windows" }), typingStatistics: statistics });
  await settingsFormReady();
  expect(screen.getByRole("button", { name: "打字统计" })).toBeTruthy();
});

test("Windows and Linux hosts expose the shared fuzzy-pinyin settings on the 输入 page", async () => {
  mount({ host: capabilities({ platform: "windows", fuzzy_pinyin: true }), fuzzyPinyin: true });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(输入|输入设置)$/ }));
  selectSettingsTab("进阶");
  expect(screen.getByRole("group", { name: "模糊音" })).toBeTruthy();

  cleanup();
  mount({ host: capabilities({ platform: "linux", fuzzy_pinyin: true }), fuzzyPinyin: true });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(输入|输入设置)$/ }));
  selectSettingsTab("进阶");
  expect(screen.getByRole("group", { name: "模糊音" })).toBeTruthy();
});

test("shortcut groups follow declared capabilities, not the platform name", async () => {
  // Windows 宿主声明能力后显示对应控件，门槛是能力而非平台名或用户代理。
  const capable = mount({
    host: capabilities({ platform: "windows", mode_switch_shortcuts: true, panel_shortcuts: true }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  expect(screen.getByRole("group", { name: "输入模式切换快捷键" })).toBeTruthy();
  // 面板快捷键是一行只读参考，桌面需先进入「按键速查」标签。
  selectSettingsTab("按键速查");
  expect(
    within(screen.getByRole("region", { name: "按键速查" })).getByText("打开屏幕键盘"),
  ).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Super+K", { selector: "kbd" })).toBeTruthy();
  capable.unmount();

  const macos = mount({ host: capabilities({ platform: "macos", panel_shortcuts: true }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  selectSettingsTab("按键速查");
  expect(screen.getByText("Ctrl+Shift+Command+K", { selector: "kbd" })).toBeTruthy();
  expect(screen.queryByText("Ctrl+Shift+Super+K", { selector: "kbd" })).toBeNull();
  macos.unmount();

  // Linux 宿主未声明这些能力时，设置与速查标签都不显示对应内容。
  mount({ host: capabilities({ platform: "linux" }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  expect(screen.queryByRole("group", { name: "输入模式切换快捷键" })).toBeNull();
  selectSettingsTab("按键速查");
  expect(screen.queryByText("打开屏幕键盘")).toBeNull();
});

test("Linux 可独立开启聚焦提示并保存，不改写切换提示", async () => {
  const save = vi.fn(async (_revision, preferences) => ({ ...initial, revision: 8, preferences }));
  mount({ host: capabilities({ platform: "linux", input_mode_hud: true }), save });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(输入|输入设置)$/ }));
  selectSettingsTab("中英模式");
  const focus = screen.getByLabelText("聚焦输入框时提示") as HTMLInputElement;
  expect(focus.checked).toBe(false);
  expect((screen.getByLabelText("中英文切换提示") as HTMLInputElement).checked).toBe(true);
  fireEvent.click(focus);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, { ...initial.preferences, input_mode_focus_hud: true });
});

test.each(["macos", "windows"] as const)("%s 不显示 Linux 聚焦提示选项", async (platform) => {
  mount({ host: capabilities({ platform, input_mode_hud: true }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(输入|输入设置)$/ }));
  selectSettingsTab("中英模式");
  expect(screen.getByLabelText("中英文切换提示")).toBeTruthy();
  expect(screen.queryByLabelText("聚焦输入框时提示")).toBeNull();
});

test("Linux number-row selection follows its capability and saves through shared preferences", async () => {
  const save = vi.fn(async (_revision, preferences) => ({ ...initial, revision: 8, preferences }));
  mount({
    host: capabilities({ platform: "linux", number_row_selection: true }),
    save,
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  const toggle = screen.getByLabelText("数字键选词") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(screen.getByText("Space", { selector: "kbd" })).toBeTruthy();
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenCalledWith(7, {
    ...initial.preferences,
    number_row_selection: false,
  });

  cleanup();
  mount({ host: capabilities({ platform: "windows", number_row_selection: false }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(快捷键|按键设置)$/ }));
  expect(screen.queryByLabelText("数字键选词")).toBeNull();
});

test("the restart action needs both the capability and an injected handler", async () => {
  const withoutHandler = mount({
    host: capabilities({ platform: "linux", restart_input_method: true }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(维护与诊断|维护诊断)$/ }));
  expect(screen.queryByRole("button", { name: "重启" })).toBeNull();
  withoutHandler.unmount();

  mount({
    host: capabilities({ platform: "linux", restart_input_method: true }),
    restartInputMethod: vi.fn(),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(维护与诊断|维护诊断)$/ }));
  expect(screen.getByRole("button", { name: "重启" })).toBeTruthy();
});

test("toolbar scale is hidden while Linux component choices remain available", async () => {
  // The Linux host stands the toolbar up as an IBus property menu: the enable
  // switch and component visibility work, but the toolbar size has no surface.
  const menuOnly = mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar_appearance: false,
      floating_toolbar_components: true,
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  expect(screen.getByLabelText("在输入法菜单显示工具栏")).toBeTruthy();
  expect(screen.queryByLabelText("在桌面显示悬浮工具栏")).toBeNull();
  expect(screen.queryByLabelText("工具栏大小")).toBeNull();
  expect(screen.getByRole("group", { name: "按钮" })).toBeTruthy();
  menuOnly.unmount();

  // A host that draws its own toolbar keeps the full set.
  mount({ host: capabilities({ platform: "macos", floating_toolbar_appearance: true }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  expect(screen.getByLabelText("在桌面显示悬浮工具栏")).toBeTruthy();
  expect(screen.getByLabelText("工具栏大小")).toBeTruthy();
  // 缩放和图标尺寸合并成了这一个控件，不再分开出现。
  expect(screen.queryByLabelText("工具栏缩放")).toBeNull();
  expect(screen.queryByLabelText("图标尺寸")).toBeNull();
});

test("the Linux toolbar page explains the menu at the top instead of previewing a floating bar", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar_appearance: false,
      floating_toolbar_components: true,
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  const toolbar = screen.getByRole("group", { name: "悬浮工具栏" });
  // Linux 不画悬浮条，没有预览；说明排在第一组「显示」之前。
  expect(toolbar.querySelector("[data-toolbar-preview]")).toBeNull();
  const note = screen.getByText(
    /^Linux 不显示悬浮工具栏窗口，工具栏以输入法菜单里的「工具栏」子菜单呈现/,
  );
  const groups = [...toolbar.querySelectorAll("[data-group-title]")];
  expect(groups.map((title) => title.textContent)).toEqual(["显示", "按钮"]);
  expect(note.compareDocumentPosition(groups[0]!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(
    screen.getByText(
      "「工具栏」子菜单：IBus 在面板的属性菜单里，Fcitx5 在托盘的状态区菜单里，需要桌面提供托盘",
    ),
  ).toBeTruthy();
  expect(screen.getByText("勾选要显示在「工具栏」子菜单里的按钮")).toBeTruthy();
  expect(screen.queryByText("颜色与明暗")).toBeNull();
});

test("the Linux toolbar page says nothing applies under GNOME Shell", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar_appearance: false,
      floating_toolbar_components: true,
      candidate_panel_limit: "gnome_shell",
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  expect(
    screen.getByText(
      "Linux 不显示悬浮工具栏窗口。当前是 GNOME 桌面，IBus 在输入源菜单里只列出输入模式和设置，没有「工具栏」子菜单，这一页的设置在当前桌面不生效。",
    ),
  ).toBeTruthy();
  expect(screen.queryByText(/仍然生效/)).toBeNull();
  // 开关和按钮组的描述跟页首说明一致，不再指向 GNOME 下不存在的「工具栏」子菜单。
  expect(
    screen.getByText("当前 GNOME 桌面的输入源菜单没有「工具栏」子菜单，这个开关在这里不生效"),
  ).toBeTruthy();
  expect(screen.getByText("勾选工具栏要列出的按钮，当前 GNOME 桌面不生效")).toBeTruthy();
  expect(screen.queryByText(/IBus 在面板的属性菜单里/)).toBeNull();
  expect(screen.queryByText("勾选要显示在「工具栏」子菜单里的按钮")).toBeNull();
});

test.each(["windows", "macos", "harmony"] as const)(
  "the %s toolbar page keeps the floating bar preview and wording",
  async (platform) => {
    // 鸿蒙按 2in1 形态给能力位（手机形态没有这一页），直接从这一页打开，三家走同一条路径。
    render(
      <SettingsPage
        initialPage="floating-toolbar"
        client={{
          load: async () => initial,
          save: vi.fn(),
          host: capabilities({
            platform,
            mobile_settings: false,
            panel_windows: true,
            floating_toolbar: true,
            floating_toolbar_appearance: true,
            floating_toolbar_components: true,
          }),
        }}
      />,
    );
    await settingsFormReady();
    const toolbar = screen.getByRole("group", { name: "悬浮工具栏" });
    expect(toolbar.querySelector("[data-toolbar-preview]")).toBeTruthy();
    expect(screen.getByLabelText("在桌面显示悬浮工具栏")).toBeTruthy();
    expect(screen.getByText("快速访问输入法状态与常用功能")).toBeTruthy();
    expect(screen.getByText("勾选要显示在悬浮工具栏上的按钮")).toBeTruthy();
    expect(screen.queryByText(/Linux 不显示悬浮工具栏窗口/)).toBeNull();
    const groups = [...toolbar.querySelectorAll("[data-group-title]")];
    expect(groups.map((title) => title.textContent)).toEqual(["显示", "按钮", "尺寸"]);
  },
);

test("the Linux toolbar page previews the floating bar its host draws and names the menu fallback", async () => {
  // Linux 宿主在 X11 上画悬浮工具栏并报告 floating_toolbar_appearance：页面与 macOS 一样有预览、尺寸和颜色，预览下另说明画不出来时退回「工具栏」子菜单。
  mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar: true,
      floating_toolbar_appearance: true,
      floating_toolbar_components: true,
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  const toolbar = screen.getByRole("group", { name: "悬浮工具栏" });
  expect(toolbar.querySelector("[data-toolbar-preview]")).toBeTruthy();
  expect(screen.getByLabelText("在桌面显示悬浮工具栏")).toBeTruthy();
  expect(screen.getByLabelText("工具栏大小")).toBeTruthy();
  expect(
    screen.getByText(/^Linux 在 X11 上显示悬浮工具栏，Wayland 桌面经 Xwayland 显示/),
  ).toBeTruthy();
  expect(screen.queryByText(/Linux 不显示悬浮工具栏窗口/)).toBeNull();
  expect(screen.queryByText(/GNOME/)).toBeNull();
  const groups = [...toolbar.querySelectorAll("[data-group-title]")];
  expect(groups.map((title) => title.textContent)).toEqual(["显示", "按钮", "尺寸"]);
});

test("the Linux toolbar page under GNOME Shell does not point at a menu fallback that is not there", async () => {
  // GNOME Shell 的 IBus 输入源菜单没有「工具栏」子菜单：画悬浮条的宿主在那里不能说退回子菜单，开关和按钮照样作用于悬浮条。
  mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar: true,
      floating_toolbar_appearance: true,
      floating_toolbar_components: true,
      candidate_panel_limit: "gnome_shell",
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ }));
  const toolbar = screen.getByRole("group", { name: "悬浮工具栏" });
  expect(toolbar.querySelector("[data-toolbar-preview]")).toBeTruthy();
  expect(
    screen.getByText(/当前是 GNOME 桌面，IBus 的输入源菜单里没有「工具栏」子菜单/),
  ).toBeTruthy();
  expect(screen.queryByText(/以输入法菜单里的「工具栏」子菜单呈现/)).toBeNull();
  expect(screen.queryByText(/Linux 不显示悬浮工具栏窗口/)).toBeNull();
  expect(screen.getByText("快速访问输入法状态与常用功能")).toBeTruthy();
  expect(screen.getByText("勾选要显示在悬浮工具栏上的按钮")).toBeTruthy();
  expect(screen.queryByText(/当前 GNOME 桌面不生效|这个开关在这里不生效/)).toBeNull();
});

test("the floating-toolbar settings page is hidden when the host has no toolbar", async () => {
  mount({
    host: capabilities({
      platform: "android",
      floating_toolbar: false,
      floating_toolbar_appearance: false,
    }),
  });
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: /^(悬浮工具栏|快捷工具)$/ })).toBeNull();
});

test("candidate appearance follows host capabilities", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_font_controls: false,
      candidate_row_colors: true,
      candidate_selection_appearance: false,
    }),
  });
  await settingsFormReady();
  expect(screen.queryByRole("radiogroup", { name: "候选字体", hidden: true })).toBeNull();
  expect(screen.queryByLabelText("主字体")).toBeNull();
  expect(screen.queryByLabelText("字号")).toBeNull();
  expect(screen.queryByLabelText("预编辑字号")).toBeNull();
  expect(screen.getByLabelText("候选强调色")).toBeTruthy();
  expect(screen.getByLabelText("候选选中色")).toBeTruthy();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
  expect(screen.getByLabelText("候选文字颜色")).toBeTruthy();
  expect(screen.getByLabelText("候选表面色")).toBeTruthy();
  // Linux 的编号色跟随主题（由文字颜色推出），不给取色器。
  expect(screen.queryByLabelText("候选编号颜色")).toBeNull();
  expect(screen.getByText("当前宿主的候选窗口不支持自定义字体或字号。")).toBeTruthy();
  expect(screen.getByText("悬停颜色不支持：Linux 的候选面板没有悬停状态。")).toBeTruthy();
});

test("Linux leaves the number and border colours to the theme and says which host the accent reaches", async () => {
  // What host_surface.rs answers for Linux: no hover state on either panel, and the number and border colours follow the theme.
  mount({
    host: capabilities({
      platform: "linux",
      candidate_row_colors: true,
      candidate_selection_appearance: false,
    }),
  });
  await settingsFormReady();
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
  expect(screen.queryByLabelText("候选编号颜色")).toBeNull();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(screen.getByText("悬停颜色不支持：Linux 的候选面板没有悬停状态。")).toBeTruthy();
  expect(screen.queryByText("当前宿主的候选窗口不支持悬停或边框颜色。")).toBeNull();
  // The classic UI theme has no accent colour, so the accent picker says it reaches IBus only.
  const captions = screen.getAllByText(
    "Fcitx5 经典界面中编号跟随正文颜色、固定候选不单独着色，此项仅对 IBus 生效",
  );
  expect(captions).toHaveLength(1);
  expect(captions[0].closest("[data-settings-search-label]")?.textContent).toContain("候选强调色");
});

// 宿主报告读边框色和编号色时（不是现在的 Linux），边框说明照旧讲清 Fcitx5 与 IBus 的区别，编号色同样标明只对 IBus 生效。
test("a Linux host that reads the border and number colours says which panel draws them", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_row_colors: true,
      candidate_selection_appearance: false,
      candidate_border_color: true,
      candidate_number_color: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("候选边框色")).toBeTruthy();
  expect(
    screen.getByText("悬停颜色不支持；边框仅在 Fcitx5 经典界面绘制，IBus 候选窗口无边框。"),
  ).toBeTruthy();
  const captions = screen.getAllByText(
    "Fcitx5 经典界面中编号跟随正文颜色、固定候选不单独着色，此项仅对 IBus 生效",
  );
  // 每条说明是对应颜色那一行的描述，紧挨着行标题；页码颜色排在强调色之前，紧跟在它所编号的文字颜色之后。
  expect(captions).toHaveLength(2);
  expect(captions[0].closest("[data-settings-search-label]")?.textContent).toContain(
    "候选编号颜色",
  );
  expect(captions[1].closest("[data-settings-search-label]")?.textContent).toContain("候选强调色");
});

// Fcitx5 在 X11 上用原生画布画候选窗时（宿主报告 `renderer: native_canvas`，`apply_linux_native_canvas` 打开选中外观），编号、强调色和固定候选都由它自己画，「仅对 IBus 生效」的说明不再成立。
test("a Linux host drawing with the native canvas does not say the accent reaches IBus only", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_row_colors: true,
      candidate_selection_appearance: true,
      candidate_number_color: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("候选强调色")).toBeTruthy();
  expect(screen.getByLabelText("候选编号颜色")).toBeTruthy();
  expect(screen.queryByText(/此项仅对 IBus 生效/)).toBeNull();
});

test.each([
  [
    "gnome_shell",
    "GNOME Shell 自己绘制 IBus 候选窗口并跟随 Shell 主题，这里的候选字体、颜色、皮肤、整体大小、圆角、内边距、不透明度和水杉 logo 在当前桌面都不会生效。",
  ],
  [
    "fcitx_theme",
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，或者设置了 MSIME_FCITX5_THEME_AUTOGENERATE=0 停止更新水杉主题，这里的候选颜色和皮肤不会写进候选窗，圆角、内边距、不透明度和水杉 logo 也不会生效，整体大小只缩放字号；字体仍然生效。把主题改回 Fcitx5 默认主题、在这里换一个全局主题，或去掉这个环境变量后重启 Fcitx5，即可让这些设置生效。",
  ],
  [
    "kimpanel",
    "Fcitx5 的候选窗口由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色、皮肤、整体大小、圆角、内边距、不透明度和水杉 logo 都不会生效。",
  ],
  [
    "ibus_panel",
    "IBus 的候选窗口由桌面的输入法面板绘制，只接收候选字体和颜色：整体大小只缩放字号，这里的圆角、内边距、不透明度、水杉 logo 和皮肤的装饰图在 IBus 下不会生效。",
  ],
] as const)(
  "a Linux panel that ignores the appearance settings (%s) is named on the appearance and skin pages",
  async (limit, note) => {
    mount({
      host: capabilities({
        platform: "linux",
        candidate_selection_appearance: false,
        candidate_border_color: true,
        candidate_panel_limit: limit,
      }),
    });
    await settingsFormReady();
    const notes = screen.getAllByText(note);
    expect(notes).toHaveLength(2);
    expect(notes.map((item) => item.closest("fieldset")?.getAttribute("aria-label"))).toEqual([
      "主题",
      "候选窗口",
    ]);
  },
);

test("a panel that honours the appearance settings gets no limit note", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_selection_appearance: false,
      candidate_border_color: true,
    }),
  });
  await settingsFormReady();
  expect(screen.queryByText(/GNOME Shell 自己绘制/)).toBeNull();
  expect(screen.queryByText(/Fcitx5 正在使用你在 Fcitx5 配置中选择的/)).toBeNull();
  expect(screen.queryByText(/Kimpanel 绘制/)).toBeNull();
  expect(screen.queryByText(/IBus 的候选窗口由桌面的输入法面板绘制/)).toBeNull();
});

test("Linux panel font takes the family and size but not a preedit size", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_font_controls: true,
      candidate_preedit_font: false,
      candidate_selection_appearance: false,
    }),
  });
  await settingsFormReady();
  // 字体框在「候选字体」选「自定义…」后才出现，字体控件在不在看预设那一行。
  expect(screen.getByRole("radiogroup", { name: "候选字体", hidden: true })).toBeTruthy();
  expect(screen.getByLabelText("字号")).toBeTruthy();
  // The application draws the composition there, so a preedit size would change nothing.
  expect(screen.queryByLabelText("预编辑字号")).toBeNull();
  expect(screen.queryByText("当前宿主的候选窗口不支持自定义字体或字号。")).toBeNull();
});

test("Windows candidate appearance keeps native controls", async () => {
  mount({
    host: capabilities({
      platform: "windows",
      candidate_font_controls: true,
      candidate_selection_appearance: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("字号")).toBeTruthy();
  expect(screen.getByLabelText("候选强调色")).toBeTruthy();
  expect(screen.getByLabelText("候选悬停色")).toBeTruthy();
  // 编号色和边框色在 Windows 上跟随主题，宿主不读这两个槽位。
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
  expect(screen.queryByLabelText("候选编号颜色")).toBeNull();
  expect(screen.queryByText(/Fcitx5 经典界面/)).toBeNull();
  // Windows places its own card, so pinning it is a real choice there.
  expect(screen.getByLabelText("跟随光标")).toBeTruthy();
});

test("macOS candidate appearance exposes the shared English face control", async () => {
  mount({
    host: capabilities({
      platform: "macos",
      candidate_font_controls: true,
      candidate_english_font: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("英文字体")).toBeTruthy();
  // 字体框在「候选字体」选「自定义…」后才出现，字体控件在不在看预设那一行。
  expect(screen.getByRole("radiogroup", { name: "候选字体", hidden: true })).toBeTruthy();
});

test("Linux candidate appearance offers the English face, which leads the panel's Pango family list", async () => {
  mount({
    host: capabilities({
      platform: "linux",
      candidate_font_controls: true,
      candidate_english_font: true,
      candidate_selection_appearance: false,
    }),
  });
  await settingsFormReady();
  const english = screen.getByLabelText("英文字体") as HTMLInputElement;
  // Unset follows the primary family, which is what the panel draws until one is chosen.
  expect(english.value).toBe("Noto Sans SC");
  expect(screen.getByText(/未设置时跟随候选主字体/)).toBeTruthy();
  // 字体框在「候选字体」选「自定义…」后才出现，字体控件在不在看预设那一行。
  expect(screen.getByRole("radiogroup", { name: "候选字体", hidden: true })).toBeTruthy();
});

// Android 的触屏候选栏画键盘皮肤的颜色，不读候选颜色：字体和字号照常给，候选颜色取色器不显示。
test("Android candidate appearance exposes native font controls but no candidate colours", async () => {
  mount({
    host: capabilities({
      platform: "android",
      system_fonts: false,
      candidate_font_controls: true,
      candidate_english_font: true,
      candidate_row_colors: true,
      candidate_selection_appearance: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("英文字体")).toBeTruthy();
  // 字体框在「候选字体」选「自定义…」后才出现，字体控件在不在看预设那一行。
  expect(screen.getByRole("radiogroup", { name: "候选字体", hidden: true })).toBeTruthy();
  expect(screen.getByLabelText("字号")).toBeTruthy();
  expect(screen.queryByLabelText("候选强调色")).toBeNull();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
});

test("a host that does not place its own card hides the follow-cursor choice", async () => {
  // IBus owns the candidate list's placement, so offering the toggle would be
  // a setting the host cannot honour.
  mount({ host: capabilities({ platform: "linux", candidate_follow_cursor: false }) });
  await settingsFormReady();
  expect(screen.queryByLabelText("跟随光标")).toBeNull();
});

test("a host with one commit path is not offered a choice between three", async () => {
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => initial,
        save: vi.fn(),
        // 2in1 画完整的语音页；手机把录音行为收成「录音」组，这里要测的是能力，不是手机的排版。
        host: capabilities({ platform: "harmony", mobile_settings: false }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByLabelText("结果提交策略")).toBeNull();
  cleanup();
  render(
    <SettingsPage
      initialPage="voice"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "windows", voice_commit_mode: true }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.getByRole("combobox", { name: "结果提交策略" })).toBeTruthy();
});

test("the shuangpin preedit choice reaches every host that draws the composition", async () => {
  // Every host's Engine honours the preference; the control belongs where the user can see the
  // difference. The HarmonyOS keyboard draws the Engine's editing text on its composition row.
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        // 有触屏方案的宿主在候选栏页设置双拼预编辑；真实的鸿蒙宿主总是带着它。
        touchKeyboardSchemes: true,
        host: capabilities({ platform: "harmony", shuangpin_preedit: true }),
      }}
    />,
  );
  expect(await screen.findByLabelText("双拼预编辑")).toBeTruthy();
  cleanup();
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "windows" }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByLabelText("双拼预编辑")).toBeNull();
});

test("the English completion switch follows the capability, and iOS keeps its own", async () => {
  // Harmony draws the completions from the packaged dictionary and reads the shared preference, so
  // the shared control belongs there. iOS offers the same surface from its native store and has a
  // separate switch, which is why it does not claim this one.
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "harmony", english_suggestions: true }),
      }}
    />,
  );
  // HarmonyOS 手机的 表达 页按设计稿把同一个开关叫作 英文联想。
  expect(await screen.findByLabelText("英文联想")).toBeTruthy();
  cleanup();
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "ios", english_suggestions: false }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByLabelText("英文建议")).toBeNull();
});

test("a host opts into the surfaces whose preferences its keyboard reads", async () => {
  // Four sections that write shared preferences and nothing else. Harmony's keyboard consumes all
  // four; without the opt-in the page offered no way to change what it was reading.
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "harmony" }),
        fuzzyPinyin: true,
        touchKeyboardSchemes: true,
        candidateEnglishGloss: true,
      }}
    />,
  );
  expect(await screen.findByLabelText("启用模糊音")).toBeTruthy();
  expect(screen.getByRole("group", { name: "输入方案" })).toBeTruthy();
  expect(screen.getByLabelText("显示英文释义")).toBeTruthy();
  cleanup();
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: capabilities({ platform: "harmony" }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByLabelText("启用模糊音")).toBeNull();
  expect(screen.queryByLabelText("显示英文释义")).toBeNull();
});

test("the candidate English font follows the capability rather than a list of platform names", async () => {
  // HarmonyOS consumes candidate_english_font, and the control used to be gated on a platform list
  // that did not include it — the preference was honoured and nobody could set it.
  const appearance = (candidate_english_font: boolean) =>
    render(
      <SettingsPage
        initialPage="appearance"
        client={{
          load: async () => initial,
          save: vi.fn(),
          host: capabilities({
            platform: "harmony",
            mobile_settings: false,
            panel_windows: true,
            candidate_font_controls: true,
            candidate_english_font,
          }),
        }}
      />,
    );
  appearance(true);
  expect(await screen.findByLabelText("英文字体")).toBeTruthy();
  cleanup();
  appearance(false);
  await screen.findByRole("radiogroup", { name: "候选字体", hidden: true });
  expect(screen.queryByLabelText("英文字体")).toBeNull();
});

test("a host that can enumerate microphones gets the picker, whatever it is called", async () => {
  // HarmonyOS records through its own capturer for the two network providers, so the choice is
  // routable there; the gate is the capability and the reader, not the platform name.
  const read = vi.fn().mockResolvedValue([{ backend: "harmony", id: "15:", label: "内置麦克风" }]);
  const voice = (voice_capture_devices: boolean) =>
    render(
      <SettingsPage
        initialPage="voice"
        client={{
          load: async () => initial,
          save: vi.fn(),
          listVoiceCaptureDevices: read,
          // 录音设备位于「录音行为」标签，这里测的是能力。HarmonyOS 只有一种录音方式（`voice_capture_backends` 为 false），只给一个「麦克风」下拉。
          host: capabilities({
            platform: "harmony",
            mobile_settings: false,
            voice_capture_devices,
            voice_capture_backends: false,
          }),
        }}
      />,
    );
  voice(true);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("tab", { name: "录音行为" }));
  expect(screen.getByRole("combobox", { name: "麦克风" })).toBeTruthy();
  expect(screen.queryByLabelText("可用录音设备")).toBeNull();
  cleanup();
  voice(false);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("tab", { name: "录音行为" }));
  expect(screen.queryByRole("combobox", { name: "麦克风" })).toBeNull();
});

test("the mode badge switch follows the capability rather than the macOS platform name", async () => {
  // HarmonyOS draws the same badge from a 2in1 status-bar panel, so the control has to reach a host
  // that is not macOS. A host that draws no badge still must not be offered a switch for one.
  mount({ host: capabilities({ platform: "harmony", input_mode_hud: true }) });
  await settingsFormReady();
  expect(screen.getByLabelText("中英文切换提示")).toBeTruthy();
  cleanup();
  mount({ host: capabilities({ platform: "macos", input_mode_hud: false }) });
  await settingsFormReady();
  expect(screen.queryByLabelText("中英文切换提示")).toBeNull();
});

test("the menu bar status item switch appears only where the host declares it", async () => {
  // 只有 macOS 宿主在菜单栏放状态项；Windows 有托盘语言栏、Linux 有输入法面板，不提供这个开关。
  mount({ host: capabilities({ platform: "macos", input_mode_status_item: true }) });
  await settingsFormReady();
  expect(screen.getByLabelText("菜单栏显示中/英状态")).toBeTruthy();
  for (const platform of ["windows", "linux"] as const) {
    cleanup();
    mount({ host: capabilities({ platform, input_mode_status_item: false }) });
    await settingsFormReady();
    expect(screen.queryByLabelText("菜单栏显示中/英状态")).toBeNull();
  }
});

test("a host whose skin folder is unreachable is offered an import, not a folder", async () => {
  // The source opens its skin folder so a skin can be dropped in. HarmonyOS keeps that folder in
  // the application sandbox where no file manager reaches it, so the button has to say what it
  // actually does — it was disabled there, promising a folder that never appeared.
  mount({
    // HarmonyOS 2in1：手机不显示外部皮肤这一行，桌面形态的 2in1 才有。
    host: capabilities({
      platform: "harmony",
      mobile_settings: false,
      skin_directory_import: true,
    }),
    scanSkinCatalog: async () => ({ directory: "/skins", packages: [], issues: [] }),
    openSkinDirectory: async () => undefined,
  });
  await settingsFormReady();
  // 2in1 是桌面形态，本地皮肤操作在「主题」的「本地皮肤」标签中。
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  selectSettingsTab("本地皮肤");
  expect(screen.getByRole("button", { name: "导入皮肤" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "打开目录" })).toBeNull();
});

test("a desktop host still opens its skin folder", async () => {
  mount({
    host: capabilities({ platform: "windows" }),
    scanSkinCatalog: async () => ({ directory: "C:/skins", packages: [], issues: [] }),
    openSkinDirectory: async () => undefined,
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  selectSettingsTab("本地皮肤");
  expect(screen.getByRole("button", { name: "打开目录" })).toBeTruthy();
});
