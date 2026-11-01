// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  CandidateFontPresetRow,
  CandidateScaleRow,
  CandidateWindowStyleSection,
  SettingsPage,
  settingsCapabilities,
  type HostPlatform,
  type Preferences,
} from "@msime/ui";
import candidateSkinHeader from "../../../../platforms/macos/src/candidate/CandidateSkin.h?raw";
import {
  candidateFontPresetAvailable,
  candidateFontPresetPatch,
  currentCandidateFontPreset,
} from "../../../../packages/ui/src/candidate/candidate-font-presets";
import { GroupList } from "../../../../packages/ui/src/core/platform-controls";
import {
  candidateCardPadding,
  candidateCornerRadius,
  candidateOpacityPercent,
  candidateScalePercent,
  candidateWindowStyle,
  candidateGeometryFields,
  candidateGeometryValue,
  candidateGeometryPatch,
} from "../../../../packages/ui/src/candidate/candidate-window-style";
import { customCandidatePalette } from "../../../../packages/ui/src/theme/global-theme";

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.macosLiquidGlass;
  vi.restoreAllMocks();
});

interface SectionOptions {
  preferences: Partial<Preferences>;
  platform: HostPlatform;
  showScale: boolean;
  showOpacity: boolean;
  showCornerRadius: boolean;
  showPadding: boolean;
  showItemGeometry: boolean;
  showFontPresets: boolean;
}

/** 本文件覆盖的几行：预设、缩放，以及 HarmonyOS 手机「候选栏」页仍用的「窗口样式」组。桌面「候选窗口」页把缩放放在「字体与大小」，不透明度和圆角放在「窗口样式」，见 `candidate-settings-layout.test.tsx`。 */
function renderSection(overrides: Partial<SectionOptions> = {}) {
  const options: SectionOptions = {
    preferences: {},
    platform: "macos",
    showScale: true,
    showOpacity: true,
    showCornerRadius: true,
    showPadding: false,
    showItemGeometry: false,
    showFontPresets: true,
    ...overrides,
  };
  const onChange = vi.fn();
  const view = render(
    <>
      <GroupList title="字体与大小">
        {options.showFontPresets && (
          <CandidateFontPresetRow
            preferences={options.preferences}
            platform={options.platform}
            onChange={onChange}
          />
        )}
        {options.showScale && (
          <CandidateScaleRow
            preferences={options.preferences}
            platform={options.platform}
            onChange={onChange}
          />
        )}
      </GroupList>
      <CandidateWindowStyleSection
        preferences={options.preferences}
        platform={options.platform}
        showOpacity={options.showOpacity}
        showCornerRadius={options.showCornerRadius}
        showPadding={options.showPadding}
        showItemGeometry={options.showItemGeometry}
        onChange={onChange}
      >
        <p>跳转行</p>
      </CandidateWindowStyleSection>
    </>,
  );
  return { onChange, view };
}

test("窗口样式 holds the opacity and the radius, then the rows the page adds after them", () => {
  renderSection();
  const group = screen.getByRole("region", { name: "窗口样式" });
  const titles = [...group.querySelectorAll("[data-row-title]")].map((row) => row.textContent);
  expect(titles).toEqual(["不透明度", "圆角大小"]);
  expect(group.lastElementChild?.lastElementChild?.textContent).toBe("跳转行");
  // 颜色和明暗现在只在「主题」页编辑。
  expect(within(group).queryByRole("combobox")).toBeNull();
  expect(group.querySelector('input[type="color"]')).toBeNull();
});

test("the sliders write the window style, leaving the defaults out of the document", () => {
  const { onChange } = renderSection();
  const scale = screen.getByRole("slider", { name: "整体大小" });
  expect(scale.getAttribute("min")).toBe("75");
  expect(scale.getAttribute("max")).toBe("150");
  expect(scale.getAttribute("step")).toBe("5");
  expect((scale as HTMLInputElement).value).toBe("100");
  fireEvent.change(scale, { target: { value: "125" } });
  expect(onChange).toHaveBeenLastCalledWith({ candidate_scale_percent: 125 });

  const opacity = screen.getByRole("slider", { name: "不透明度" });
  expect(opacity.getAttribute("min")).toBe("50");
  expect(opacity.getAttribute("max")).toBe("100");
  fireEvent.change(opacity, { target: { value: "80" } });
  expect(onChange).toHaveBeenLastCalledWith({ candidate_opacity_percent: 80 });

  const radius = screen.getByRole("slider", { name: "圆角大小" });
  expect(radius.getAttribute("max")).toBe("16");
  fireEvent.change(radius, { target: { value: "12" } });
  expect(onChange).toHaveBeenLastCalledWith({ candidate_corner_radius: 12 });

  cleanup();
  const edited = renderSection({
    preferences: { candidate_scale_percent: 125, candidate_opacity_percent: 80 },
  });
  fireEvent.change(screen.getByRole("slider", { name: "整体大小" }), {
    target: { value: "100" },
  });
  expect(edited.onChange).toHaveBeenLastCalledWith({ candidate_scale_percent: undefined });
  fireEvent.change(screen.getByRole("slider", { name: "不透明度" }), {
    target: { value: "100" },
  });
  expect(edited.onChange).toHaveBeenLastCalledWith({ candidate_opacity_percent: undefined });
});

// #10137：触屏键盘上叫「候选栏大小」，三端同一说法，给出核心接受的整个 50–200，回到 100% 时从文档里删掉。
test.each([
  ["android", false],
  ["ios", false],
  ["harmony", true],
] as const)("%s names the touch scale 候选栏大小 over the whole core range", (platform, touch) => {
  const onChange = vi.fn();
  render(
    <GroupList title="候选栏">
      <CandidateScaleRow
        preferences={{ candidate_scale_percent: 80 }}
        platform={platform}
        touch={touch || undefined}
        onChange={onChange}
      />
    </GroupList>,
  );
  const scale = screen.getByRole("slider", { name: "候选栏大小" });
  expect(scale.getAttribute("min")).toBe("50");
  expect(scale.getAttribute("max")).toBe("200");
  expect(scale.getAttribute("step")).toBe("5");
  // 数值由滑块的读数显示，说明只讲缩放什么。
  expect(scale.getAttribute("aria-valuetext")).toBe("80%");
  expect(screen.getByText("候选字和工具栏一起缩放，按键大小不变")).toBeTruthy();
  fireEvent.change(scale, { target: { value: "50" } });
  expect(onChange).toHaveBeenLastCalledWith({ candidate_scale_percent: 50 });
  fireEvent.change(scale, { target: { value: "100" } });
  expect(onChange).toHaveBeenLastCalledWith({ candidate_scale_percent: undefined });
  expect(screen.queryByRole("slider", { name: "整体大小" })).toBeNull();
});

// 鸿蒙 2in1 的候选卡片仍叫「整体大小」：桌面形态不传 `touch`。
test("a HarmonyOS 2in1 card keeps the desktop wording", () => {
  renderSection({ platform: "harmony" });
  expect(screen.getByRole("slider", { name: "整体大小" }).getAttribute("min")).toBe("75");
});

// Linux 的 Fcitx5 主题也按整体大小缩放卡片几何（IBus 面板只缩放字号，由面板限制说明交代），所以和其他宿主同一句说明。
test("Linux describes overall size like the other hosts", () => {
  renderSection({ platform: "linux" });
  expect(screen.getByText("字号与窗口尺寸一起缩放")).toBeTruthy();
});

test("the corner radius follows the skin until set, and returns to it", () => {
  const following = renderSection();
  const reset = screen.getByRole("button", { name: "跟随皮肤" });
  expect(reset.getAttribute("aria-pressed")).toBe("true");
  // 当前值（跟随皮肤或点数）常驻标题下方，覆盖皮肤这句解释收进「?」。
  expect(screen.getByText("设置后覆盖皮肤自带的圆角")).toBeTruthy();
  expect(screen.queryByText("10pt")).toBeNull();
  fireEvent.click(reset);
  expect(following.onChange).not.toHaveBeenCalled();

  cleanup();
  const set = renderSection({ preferences: { candidate_corner_radius: 10 } });
  expect(screen.getByText("10pt")).toBeTruthy();
  const button = screen.getByRole("button", { name: "跟随皮肤" });
  expect(button.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(button);
  expect(set.onChange).toHaveBeenCalledWith({ candidate_corner_radius: undefined });
});

/** 原生 macOS 候选窗的卡片内边距（`SkinTokens::pad`），也是样式表 macOS 默认值块的 `--msime-host-pad`。 */
const macosNativePad = String(Number(/float pad = ([\d.]+)f;/.exec(candidateSkinHeader)![1]));

// #7316：内边距跟在圆角后面，跟随皮肤时滑块停在 macOS 原生的 4pt；滑块覆盖核心的整个 0–16（#7974），macOS 卡片边缘至少留出边框宽度由宿主保证。
test("the card padding follows the skin until set, writes the chosen points and returns to the skin", () => {
  const following = renderSection({ showPadding: true });
  const group = screen.getByRole("region", { name: "窗口样式" });
  const titles = [...group.querySelectorAll("[data-row-title]")].map((row) => row.textContent);
  expect(titles).toEqual(["不透明度", "圆角大小", "内边距"]);
  const slider = screen.getByRole("slider", { name: "内边距" }) as HTMLInputElement;
  expect(slider.getAttribute("min")).toBe("0");
  expect(slider.getAttribute("max")).toBe("16");
  expect(slider.value).toBe(macosNativePad);
  expect(
    screen.getByText("调小后候选离窗口边缘更近，窗口更紧凑；设置后覆盖皮肤自带的内边距"),
  ).toBeTruthy();
  expect(screen.queryByText("3pt")).toBeNull();
  fireEvent.change(slider, { target: { value: "1" } });
  expect(following.onChange).toHaveBeenLastCalledWith({ candidate_padding: 1 });

  cleanup();
  const set = renderSection({
    showPadding: true,
    preferences: { candidate_corner_radius: 10, candidate_padding: 3 },
  });
  expect(screen.getByText("3pt")).toBeTruthy();
  expect((screen.getByRole("slider", { name: "内边距" }) as HTMLInputElement).value).toBe("3");
  const resets = screen.getAllByRole("button", { name: "跟随皮肤" });
  expect(resets).toHaveLength(2);
  fireEvent.click(resets[1]!);
  expect(set.onChange).toHaveBeenLastCalledWith({ candidate_padding: undefined });
});

/** 原生 macOS 候选窗的卡片圆角（`SkinTokens::radius`），也是样式表 macOS 默认值块的 `--msime-host-card-radius`（`candidate-preview-native-tokens.test.ts` 对照）。 */
const macosNativeRadius = String(Number(/float radius = ([\d.]+)f;/.exec(candidateSkinHeader)![1]));

test.each([
  ["macos", macosNativeRadius],
  ["windows", "6"],
] as const)(
  "following the skin, the %s radius slider shows the radius the preview draws",
  (platform, value) => {
    renderSection({ platform });
    expect((screen.getByRole("slider", { name: "圆角大小" }) as HTMLInputElement).value).toBe(
      value,
    );
  },
);

// Linux 的滑块停在 Fcitx5 卡片自己的 10px 圆角和 4px 内边距上（`CandidateFcitxTheme.h` 的 `FcitxPanelGeometry`）：从这里往上拖，卡片跟着变圆、变宽松，而不是先跳到更小的值。
test("following the skin on Linux, the sliders rest on the Fcitx5 card's own radius and padding", () => {
  const following = renderSection({ platform: "linux", showPadding: true });
  expect((screen.getByRole("slider", { name: "圆角大小" }) as HTMLInputElement).value).toBe("10");
  const padding = screen.getByRole("slider", { name: "内边距" }) as HTMLInputElement;
  expect(padding.value).toBe("4");
  fireEvent.change(padding, { target: { value: "5" } });
  expect(following.onChange).toHaveBeenLastCalledWith({ candidate_padding: 5 });
});

test.each([
  ["macos", macosNativeRadius],
  ["windows", "6"],
] as const)(
  "the 候选窗口 page on a %s host passes its platform to the radius slider",
  async (platform, value) => {
    render(
      <SettingsPage
        initialPage="appearance"
        client={{
          load: vi.fn().mockResolvedValue({
            format_version: 1,
            revision: 1,
            preferences: {
              scheme: "quanpin",
              shuangpin_profile: "xiaohe",
              candidate_page_size: 5,
              learning: true,
              chinese_punctuation: true,
            },
          }),
          save: vi.fn(),
          host: testHost({ platform, candidate_corner_radius: true }),
        }}
      />,
    );
    fireEvent.click(await screen.findByRole("tab", { name: "窗口样式" }));
    const slider = (await screen.findByRole("slider", { name: "圆角大小" })) as HTMLInputElement;
    expect(slider.value).toBe(value);
  },
);

test("the rows a host cannot draw are hidden", () => {
  renderSection({
    showScale: false,
    showOpacity: false,
    showCornerRadius: false,
    showFontPresets: false,
  });
  for (const title of ["整体大小", "不透明度", "圆角大小", "内边距", "候选字体"])
    expect(screen.queryByText(title)).toBeNull();
  expect(screen.getByRole("region", { name: "窗口样式" })).toBeTruthy();
});

test.each([
  ["windows", { scale: true, opacity: true, radius: true, padding: false }],
  ["macos", { scale: true, opacity: true, radius: true, padding: true }],
  ["linux", { scale: true, opacity: true, radius: true, padding: true }],
  ["harmony", { scale: false, opacity: true, radius: true, padding: false }],
  ["ios", { scale: true, opacity: false, radius: false, padding: false }],
  ["android", { scale: true, opacity: false, radius: false, padding: false }],
] as const)("%s gates the window style rows on its capabilities", (platform, expected) => {
  const host = testHost({
    platform,
    candidate_window_scale: expected.scale,
    candidate_window_opacity: expected.opacity,
    candidate_corner_radius: expected.radius,
    candidate_padding: expected.padding,
  });
  const capabilities = settingsCapabilities({
    host,
    linux: platform === "linux",
    android: platform === "android",
    ios: platform === "ios",
    harmony: platform === "harmony",
    windows: platform === "windows",
    macos: platform === "macos",
    mobile: platform === "ios" || platform === "android",
    canRestartInputMethod: false,
    canInstallInputSource: false,
    canListVoiceCaptureDevices: false,
  });
  expect({
    scale: capabilities.showCandidateWindowScale,
    opacity: capabilities.showCandidateWindowOpacity,
    radius: capabilities.showCandidateCornerRadius,
    padding: capabilities.showCandidatePadding,
  }).toEqual(expected);
});

test("the font presets write the platform's family first and lead the fallback chain with the preset", () => {
  expect(
    candidateFontPresetPatch("song", "macos", {
      candidate_fallback_fonts: ["Noto Sans SC", "Microsoft YaHei", "SimSun"],
    }),
  ).toEqual({
    candidate_font_family: "Songti SC",
    candidate_fallback_fonts: [
      "Songti SC",
      "SimSun",
      "Noto Serif CJK SC",
      "Noto Serif SC",
      "Noto Sans SC",
      "Microsoft YaHei",
    ],
  });
  expect(candidateFontPresetPatch("hei", "windows", {}).candidate_fallback_fonts).toEqual([
    "Microsoft YaHei",
    "PingFang SC",
    "Noto Sans CJK SC",
    "Noto Sans SC",
  ]);
  expect(candidateFontPresetPatch("kai", "linux", {}).candidate_font_family).toBe("AR PL UKai CN");
  expect(candidateFontPresetPatch("default", "macos", {})).toEqual({
    candidate_font_family: "Noto Sans SC",
    candidate_fallback_fonts: ["Noto Sans SC", "Microsoft YaHei"],
  });
});

test("the fallback chain keeps the core's 32-font limit", () => {
  const fonts = Array.from({ length: 32 }, (_, index) => `Font ${index}`);
  const patch = candidateFontPresetPatch("song", "windows", { candidate_fallback_fonts: fonts });
  expect(patch.candidate_fallback_fonts).toHaveLength(32);
  expect(patch.candidate_fallback_fonts?.slice(0, 4)).toEqual([
    "SimSun",
    "Songti SC",
    "Noto Serif CJK SC",
    "Noto Serif SC",
  ]);
});

test("the current preset is read back from the main font, and anything else is custom", () => {
  expect(currentCandidateFontPreset({}, "macos")).toBe("default");
  for (const id of ["song", "hei", "kai", "yuan"] as const)
    for (const platform of ["macos", "windows", "linux", "android", "harmony"] as const)
      expect(currentCandidateFontPreset(candidateFontPresetPatch(id, platform, {}), platform)).toBe(
        id,
      );
  expect(currentCandidateFontPreset({ candidate_font_family: "Comic Sans MS" }, "macos")).toBe(
    null,
  );
  // Another preset's family is still that preset whatever the fallback chain says.
  expect(currentCandidateFontPreset({ candidate_font_family: "KaiTi" }, "macos")).toBe("kai");
  // Microsoft YaHei is in the default fallback list but is 黑体's Windows face: chosen on Windows and read on another host, or after the fallbacks were reordered, it is still 黑体.
  expect(currentCandidateFontPreset({ candidate_font_family: "Microsoft YaHei" }, "macos")).toBe(
    "hei",
  );
  expect(
    currentCandidateFontPreset(
      {
        candidate_font_family: "Microsoft YaHei",
        candidate_fallback_fonts: ["Menlo", "Microsoft YaHei"],
      },
      "windows",
    ),
  ).toBe("hei");
});

test("圆体 is unavailable on Windows and the control disables it there", () => {
  expect(candidateFontPresetAvailable("yuan", "windows")).toBe(false);
  expect(candidateFontPresetAvailable("yuan", "macos")).toBe(true);
  renderSection({ platform: "windows" });
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  expect(within(presets).getByRole("radio", { name: "圆体" })).toHaveProperty("disabled", true);
  expect(within(presets).getByRole("radio", { name: "宋体" })).toHaveProperty("disabled", false);
});

test("the preset control shows the current preset and writes the chosen one", () => {
  const { onChange } = renderSection({ preferences: { candidate_font_family: "Kaiti SC" } });
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  expect(within(presets).getByRole("radio", { name: "楷体" })).toHaveProperty("checked", true);
  fireEvent.click(within(presets).getByRole("radio", { name: "黑体" }));
  expect(onChange).toHaveBeenCalledWith(
    expect.objectContaining({ candidate_font_family: "PingFang SC" }),
  );
});

test("a custom main font selects no preset and says so", () => {
  renderSection({ preferences: { candidate_font_family: "Comic Sans MS" } });
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  for (const radio of within(presets).getAllByRole("radio"))
    expect(radio).toHaveProperty("checked", false);
  expect(screen.getByText(/当前为自定义字体 Comic Sans MS/)).toBeTruthy();
});

test("the window style values are bounded like the core's validation", () => {
  expect(candidateScalePercent(undefined)).toBe(100);
  expect(candidateScalePercent(200)).toBe(200);
  expect(candidateScalePercent(201)).toBe(100);
  expect(candidateOpacityPercent(49)).toBe(100);
  expect(candidateOpacityPercent(50)).toBe(50);
  expect(candidateCornerRadius(null)).toBe(null);
  expect(candidateCornerRadius(32)).toBe(32);
  expect(candidateCornerRadius(33)).toBe(null);
  expect(candidateCardPadding(undefined)).toBe(null);
  expect(candidateCardPadding(0)).toBe(0);
  expect(candidateCardPadding(16)).toBe(16);
  expect(candidateCardPadding(17)).toBe(null);
  expect(candidateCardPadding(2.5)).toBe(null);
});

test("the preview style carries the scale, the opacity and a set radius only", () => {
  expect(candidateWindowStyle({})).toEqual({
    "--msime-candidate-scale": "1",
    "--msime-candidate-opacity": "100%",
  });
  expect(
    candidateWindowStyle({
      candidate_scale_percent: 125,
      candidate_opacity_percent: 70,
      candidate_corner_radius: 0,
    }),
  ).toEqual({
    "--msime-candidate-scale": "1.25",
    "--msime-candidate-opacity": "70%",
    "--msime-skin-radius": "0px",
    "--msime-candidate-user-radius": "0px",
  });
  expect(candidateWindowStyle({ candidate_padding: 2 })).toEqual({
    "--msime-candidate-scale": "1",
    "--msime-candidate-opacity": "100%",
    "--msime-candidate-user-pad": "2px",
  });
});

test.each([0, 4, 16])("候选项间距 %s 只改变间隔，不改变跟随皮肤时的行外边距", (spacing) => {
  expect(candidateWindowStyle({ candidate_item_spacing: spacing })).toEqual({
    ...candidateWindowStyle({}),
    "--msime-candidate-item-spacing": `${spacing}px`,
  });
});

test("分轴窗口留白与候选项几何分别显示并允许显式零值", () => {
  const { onChange } = renderSection({
    showPadding: true,
    showItemGeometry: true,
    preferences: { candidate_item_spacing: 1 },
  });
  expect(screen.queryByRole("slider", { name: "内边距" })).toBeNull();
  for (const field of candidateGeometryFields) {
    const slider = screen.getByRole("slider", { name: field.title });
    expect(slider.getAttribute("min")).toBe("0");
    expect(slider.getAttribute("max")).toBe(String(field.max));
    fireEvent.change(slider, { target: { value: "0" } });
    expect(onChange).toHaveBeenLastCalledWith(candidateGeometryPatch({}, field.key, 0));
    expect(candidateGeometryValue({ [field.key]: 0 }, field.key)).toBe(0);
    expect(candidateGeometryValue({ [field.key]: field.max + 1 }, field.key)).toBeNull();
  }
});

test("编辑或恢复单轴时拆分旧窗口留白，另一轴的视觉值不变", () => {
  const preferences = { candidate_padding: 7 };
  expect(candidateGeometryValue(preferences, "candidate_padding_vertical")).toBe(7);
  expect(candidateGeometryPatch(preferences, "candidate_padding_horizontal", 2)).toEqual({
    candidate_padding: undefined,
    candidate_padding_horizontal: 2,
    candidate_padding_vertical: 7,
  });
  expect(candidateGeometryPatch(preferences, "candidate_padding_vertical", null)).toEqual({
    candidate_padding: undefined,
    candidate_padding_vertical: undefined,
    candidate_padding_horizontal: 7,
  });
  const { onChange } = renderSection({
    showPadding: true,
    showItemGeometry: true,
    preferences,
  });
  const row = screen
    .getByText("窗口垂直内边距")
    .closest<HTMLElement>("[data-settings-search-label]")!;
  fireEvent.click(within(row).getByRole("button", { name: "跟随皮肤" }));
  expect(onChange).toHaveBeenLastCalledWith(
    candidateGeometryPatch(preferences, "candidate_padding_vertical", null),
  );
});

test("预览保留独立几何与显式零值，不把旧统一留白复制成新字段", () => {
  const style = candidateWindowStyle({
    candidate_padding_horizontal: 1,
    candidate_padding_vertical: 12,
    candidate_item_padding_horizontal: 0,
    candidate_item_padding_vertical: 3,
    candidate_item_corner_radius: 0,
    candidate_item_spacing: 6,
  });
  expect(style).toMatchObject({
    "--msime-candidate-padding-horizontal": "1px",
    "--msime-candidate-padding-vertical": "12px",
    "--msime-candidate-item-padding-horizontal": "0px",
    "--msime-candidate-item-padding-vertical": "3px",
    "--msime-candidate-item-corner-radius": "0px",
    "--msime-candidate-item-spacing": "6px",
    "--msime-candidate-preview-text-pad-left": "0px",
  });
});

test("a picked selection colour gives its text black or white by luminance", () => {
  const light = customCandidatePalette("light", { selected: "#ffe680", accent: "#112233" });
  expect(light?.selected_text).toBe("#000000");
  expect(light?.selected_number).toBe("#0000009D");
  expect(light?.accent).toBe("#112233");
  const dark = customCandidatePalette("light", { selected: "#1a3d7c" });
  expect(dark?.selected_text).toBe("#FFFFFF");
  expect(dark?.selected_number).toBe("#FFFFFF9D");
  // Over system nothing derives from the accent, but the picked selection still brings its text.
  const system = customCandidatePalette("system", { selected: "#1a3d7c" });
  expect(system?.selected_text).toBe("#FFFFFF");
  // Without the picker the selected text still follows the accent over a built-in base.
  expect(customCandidatePalette("light", { accent: "#112233" })?.selected_text).toBe("#112233");
});

test("系统玻璃的不透明度由原生材质管理，辅助功能回退后恢复编辑", async () => {
  document.documentElement.dataset.macosLiquidGlass = "true";
  const { onChange } = renderSection({
    preferences: { global_theme: "system", candidate_opacity_percent: 75 },
  });
  const slider = screen.getByRole("slider", { name: "不透明度" }) as HTMLInputElement;
  expect(slider.disabled).toBe(true);
  expect(
    screen.getByText("系统液态玻璃的不透明度由 macOS 管理；自定义主题可单独调整"),
  ).toBeTruthy();
  await act(async () => {
    document.documentElement.dataset.macosLiquidGlass = "false";
  });
  expect(slider.disabled).toBe(false);
  expect(slider.value).toBe("75");
  expect(onChange).not.toHaveBeenCalled();
});

test("自定义主题的透明度仍可独立编辑", () => {
  document.documentElement.dataset.macosLiquidGlass = "true";
  renderSection({ preferences: { global_theme: "custom", candidate_opacity_percent: 75 } });
  expect((screen.getByRole("slider", { name: "不透明度" }) as HTMLInputElement).disabled).toBe(
    false,
  );
});
