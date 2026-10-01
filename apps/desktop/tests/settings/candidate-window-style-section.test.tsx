// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import {
  CandidateFontPresetRow,
  CandidateScaleRow,
  CandidateWindowStyleSection,
  settingsCapabilities,
  type HostCapabilities,
  type HostPlatform,
  type Preferences,
} from "@msime/ui";
import {
  candidateFontPresetAvailable,
  candidateFontPresetPatch,
  currentCandidateFontPreset,
} from "../../../../packages/ui/src/candidate/candidate-font-presets";
import { GroupList } from "../../../../packages/ui/src/core/platform-controls";
import {
  candidateCornerRadius,
  candidateOpacityPercent,
  candidateScalePercent,
  candidateWindowStyle,
} from "../../../../packages/ui/src/candidate/candidate-window-style";
import { customCandidatePalette } from "../../../../packages/ui/src/theme/global-theme";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

interface SectionOptions {
  preferences: Partial<Preferences>;
  platform: HostPlatform;
  showScale: boolean;
  showOpacity: boolean;
  showCornerRadius: boolean;
  showFontPresets: boolean;
}

/** The rows this file covers as the 候选窗口 page places them: the preset and the scale in 字体与大小, the opacity and the radius in 窗口样式. */
function renderSection(overrides: Partial<SectionOptions> = {}) {
  const options: SectionOptions = {
    preferences: {},
    platform: "macos",
    showScale: true,
    showOpacity: true,
    showCornerRadius: true,
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
          <CandidateScaleRow preferences={options.preferences} onChange={onChange} />
        )}
      </GroupList>
      <CandidateWindowStyleSection
        preferences={options.preferences}
        showOpacity={options.showOpacity}
        showCornerRadius={options.showCornerRadius}
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
  // The colours and the light/dark are edited only on the 主题 page now.
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

test("the corner radius follows the skin until set, and returns to it", () => {
  const following = renderSection();
  const reset = screen.getByRole("button", { name: "跟随皮肤" });
  expect(reset.getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByText("跟随皮肤；设置后覆盖皮肤自带的圆角")).toBeTruthy();
  fireEvent.click(reset);
  expect(following.onChange).not.toHaveBeenCalled();

  cleanup();
  const set = renderSection({ preferences: { candidate_corner_radius: 10 } });
  expect(screen.getByText("10pt，覆盖皮肤自带的圆角")).toBeTruthy();
  const button = screen.getByRole("button", { name: "跟随皮肤" });
  expect(button.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(button);
  expect(set.onChange).toHaveBeenCalledWith({ candidate_corner_radius: undefined });
});

test("the rows a host cannot draw are hidden", () => {
  renderSection({
    showScale: false,
    showOpacity: false,
    showCornerRadius: false,
    showFontPresets: false,
  });
  for (const title of ["整体大小", "不透明度", "圆角大小", "候选字体"])
    expect(screen.queryByText(title)).toBeNull();
  expect(screen.getByRole("region", { name: "窗口样式" })).toBeTruthy();
});

test.each([
  ["windows", { scale: true, opacity: true, radius: true }],
  ["macos", { scale: true, opacity: true, radius: true }],
  ["linux", { scale: false, opacity: false, radius: true }],
  ["harmony", { scale: false, opacity: true, radius: true }],
  ["ios", { scale: false, opacity: false, radius: false }],
  ["android", { scale: false, opacity: false, radius: false }],
] as const)("%s gates the window style rows on its capabilities", (platform, expected) => {
  const host = {
    platform,
    candidate_window_scale: expected.scale,
    candidate_window_opacity: expected.opacity,
    candidate_corner_radius: expected.radius,
  } as HostCapabilities;
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
  }).toEqual(expected);
});

test("a host older than the style capabilities hides the rows", () => {
  const capabilities = settingsCapabilities({
    host: { platform: "windows" } as HostCapabilities,
    linux: false,
    android: false,
    ios: false,
    harmony: false,
    windows: true,
    macos: false,
    mobile: false,
    canRestartInputMethod: false,
    canInstallInputSource: false,
    canListVoiceCaptureDevices: false,
  });
  expect(capabilities.showCandidateWindowScale).toBe(false);
  expect(capabilities.showCandidateWindowOpacity).toBe(false);
  expect(capabilities.showCandidateCornerRadius).toBe(false);
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
