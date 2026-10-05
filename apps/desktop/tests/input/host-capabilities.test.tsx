// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
    candidate_border_color: overrides.candidate_selection_appearance ?? true,
    candidate_window_scale: false,
    candidate_window_opacity: false,
    candidate_corner_radius: false,
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
  await screen.findByLabelText("输入模式");
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
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.getByRole("group", { name: "模糊音" })).toBeTruthy();

  cleanup();
  mount({ host: capabilities({ platform: "linux", fuzzy_pinyin: true }), fuzzyPinyin: true });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "输入" }));
  expect(screen.getByRole("group", { name: "模糊音" })).toBeTruthy();
});

test("shortcut groups follow declared capabilities, not the platform name", async () => {
  // A Windows host that declares the capabilities gets the controls, proving the
  // gate is the capability and not a platform-name or user-agent match.
  const capable = mount({
    host: capabilities({ platform: "windows", mode_switch_shortcuts: true, panel_shortcuts: true }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(screen.getByRole("group", { name: "面板快捷键" })).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Super+K", { selector: "kbd" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "输入模式切换快捷键" })).toBeTruthy();
  capable.unmount();

  const macos = mount({ host: capabilities({ platform: "macos", panel_shortcuts: true }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(screen.getByText("Ctrl+Shift+Command+K", { selector: "kbd" })).toBeTruthy();
  expect(screen.queryByText("Ctrl+Shift+Super+K", { selector: "kbd" })).toBeNull();
  macos.unmount();

  // A Linux host that does not declare them keeps them hidden.
  mount({ host: capabilities({ platform: "linux" }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(screen.queryByRole("group", { name: "面板快捷键" })).toBeNull();
  expect(screen.queryByRole("group", { name: "输入模式切换快捷键" })).toBeNull();
});

test("Linux number-row selection follows its capability and saves through shared preferences", async () => {
  const save = vi.fn(async (_revision, preferences) => ({ ...initial, revision: 8, preferences }));
  mount({
    host: capabilities({ platform: "linux", number_row_selection: true }),
    save,
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
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
  fireEvent.click(screen.getByRole("button", { name: "快捷键" }));
  expect(screen.queryByLabelText("数字键选词")).toBeNull();
});

test("the restart action needs both the capability and an injected handler", async () => {
  const withoutHandler = mount({
    host: capabilities({ platform: "linux", restart_input_method: true }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(screen.queryByRole("button", { name: "重启" })).toBeNull();
  withoutHandler.unmount();

  mount({
    host: capabilities({ platform: "linux", restart_input_method: true }),
    restartInputMethod: vi.fn(),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "维护与诊断" }));
  expect(screen.getByRole("button", { name: "重启" })).toBeTruthy();
});

test("toolbar scale is hidden while Linux component choices remain available", async () => {
  // The Linux host stands the toolbar up as an IBus property menu: the enable
  // switch and component visibility work, but scale and icon size have no surface.
  const menuOnly = mount({
    host: capabilities({
      platform: "linux",
      floating_toolbar_appearance: false,
      floating_toolbar_components: true,
    }),
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  expect(screen.getByLabelText("在桌面显示悬浮工具栏")).toBeTruthy();
  expect(screen.queryByLabelText("工具栏缩放")).toBeNull();
  expect(screen.queryByLabelText("图标尺寸")).toBeNull();
  expect(screen.getByRole("group", { name: "按钮" })).toBeTruthy();
  menuOnly.unmount();

  // A host that draws its own toolbar keeps the full set.
  mount({ host: capabilities({ platform: "macos", floating_toolbar_appearance: true }) });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "悬浮工具栏" }));
  expect(screen.getByLabelText("工具栏缩放")).toBeTruthy();
  expect(screen.getByLabelText("图标尺寸")).toBeTruthy();
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
  expect(screen.queryByRole("button", { name: "悬浮工具栏" })).toBeNull();
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
  expect(screen.queryByLabelText("主字体")).toBeNull();
  expect(screen.queryByLabelText("字号")).toBeNull();
  expect(screen.queryByLabelText("预编辑字号")).toBeNull();
  expect(screen.getByLabelText("候选强调色")).toBeTruthy();
  expect(screen.getByLabelText("候选选中色")).toBeTruthy();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(screen.queryByLabelText("候选边框色")).toBeNull();
  expect(screen.getByLabelText("候选文字颜色")).toBeTruthy();
  expect(screen.getByLabelText("候选表面色")).toBeTruthy();
  expect(screen.getByLabelText("候选编号颜色")).toBeTruthy();
  expect(screen.getByText("当前宿主的候选窗口不支持自定义字体或字号。")).toBeTruthy();
  expect(
    screen.getByText("悬停颜色不支持；边框仅在 Fcitx5 经典界面绘制，IBus 候选窗口无边框。"),
  ).toBeTruthy();
});

test("Linux offers the border colour Fcitx5 draws and says which host each colour reaches", async () => {
  // What host_surface.rs answers for Linux: no hover state on either panel, but the Fcitx5 classic UI theme carries the border.
  mount({
    host: capabilities({
      platform: "linux",
      candidate_row_colors: true,
      candidate_selection_appearance: false,
      candidate_border_color: true,
    }),
  });
  await settingsFormReady();
  expect(screen.getByLabelText("候选边框色")).toBeTruthy();
  expect(screen.queryByLabelText("候选悬停色")).toBeNull();
  expect(
    screen.getByText("悬停颜色不支持；边框仅在 Fcitx5 经典界面绘制，IBus 候选窗口无边框。"),
  ).toBeTruthy();
  expect(screen.queryByText("当前宿主的候选窗口不支持悬停或边框颜色。")).toBeNull();
  // The classic UI theme has no label or accent colour, so both pickers say they reach IBus only.
  const captions = screen.getAllByText(
    "Fcitx5 经典界面中编号跟随正文颜色、固定候选不单独着色，此项仅对 IBus 生效",
  );
  expect(captions).toHaveLength(2);
  // 每条说明是对应颜色那一行的描述，紧挨着行标题；页码颜色排在强调色之前，紧跟在它所编号的文字颜色之后。
  expect(captions[0].parentElement?.textContent).toContain("候选编号颜色");
  expect(captions[1].parentElement?.textContent).toContain("候选强调色");
});

test.each([
  [
    "gnome_shell",
    "GNOME Shell 自己绘制 IBus 候选窗口并跟随 Shell 主题，这里的候选字体、颜色和皮肤在当前桌面不会生效。",
  ],
  [
    "fcitx_theme",
    "Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题，这里的候选颜色和皮肤不会覆盖它；字体仍然生效。改回 Fcitx5 默认主题后即可使用这里的设置。",
  ],
  [
    "kimpanel",
    "Fcitx5 的候选窗口由桌面的 Kimpanel 绘制，使用桌面自己的字体和主题，这里的候选字体、颜色和皮肤不会生效。",
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
  expect(screen.getByLabelText("主字体")).toBeTruthy();
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
  expect(screen.getByLabelText("候选边框色")).toBeTruthy();
  expect(screen.getByLabelText("候选悬停色")).toBeTruthy();
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
  expect(screen.getByLabelText("主字体")).toBeTruthy();
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
  expect(screen.getByLabelText("主字体")).toBeTruthy();
});

test("Android candidate appearance exposes native font and color controls", async () => {
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
  expect(screen.getByLabelText("主字体")).toBeTruthy();
  expect(screen.getByLabelText("字号")).toBeTruthy();
  expect(screen.getByLabelText("候选强调色")).toBeTruthy();
  expect(screen.getByLabelText("候选悬停色")).toBeTruthy();
  expect(screen.getByLabelText("候选边框色")).toBeTruthy();
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
        host: capabilities({ platform: "harmony" }),
      }}
    />,
  );
  await screen.findByText("录音行为");
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
  expect(await screen.findByLabelText("结果提交策略")).toBeTruthy();
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
  expect(await screen.findByLabelText("英文建议")).toBeTruthy();
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
  await screen.findByLabelText("主字体");
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
          host: capabilities({ platform: "harmony", voice_capture_devices }),
        }}
      />,
    );
  voice(true);
  expect(await screen.findByLabelText("可用录音设备")).toBeTruthy();
  cleanup();
  voice(false);
  await screen.findByText("录音行为");
  expect(screen.queryByLabelText("可用录音设备")).toBeNull();
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

test("a host whose skin folder is unreachable is offered an import, not a folder", async () => {
  // The source opens its skin folder so a skin can be dropped in. HarmonyOS keeps that folder in
  // the application sandbox where no file manager reaches it, so the button has to say what it
  // actually does — it was disabled there, promising a folder that never appeared.
  mount({
    host: capabilities({ platform: "harmony", skin_directory_import: true }),
    scanSkinCatalog: async () => ({ directory: "/skins", packages: [], issues: [] }),
    openSkinDirectory: async () => undefined,
  });
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
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
  expect(screen.getByRole("button", { name: "打开目录" })).toBeTruthy();
});
