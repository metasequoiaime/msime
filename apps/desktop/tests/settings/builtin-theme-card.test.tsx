// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { BuiltinThemeCard, type BuiltinThemeCardProps, themeEntry } from "@msime/ui";
import { defaultTouchKeyboardSkinDesign } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-design";

afterEach(cleanup);

function props(patch: Partial<BuiltinThemeCardProps> = {}): BuiltinThemeCardProps {
  return {
    entry: themeEntry("system"),
    globalTheme: "system",
    packageSelected: false,
    preferences: {},
    candidatePreviewTheme: "light",
    keyboardDesign: defaultTouchKeyboardSkinDesign,
    mobile: false,
    linux: false,
    onPreferencesChange: vi.fn(),
    onTogglePreview: vi.fn(),
    ...patch,
  };
}

test("固定明暗主题忽略预览覆盖，使用开关应用主题", () => {
  const options = props({ entry: themeEntry("night"), previewTheme: "light" });
  const { container } = render(<BuiltinThemeCard {...options} />);
  const control = screen.getByRole("switch", { name: options.entry.title });
  expect(control.getAttribute("aria-checked")).toBe("false");
  expect(screen.getByText(`${options.entry.title}（深色）`)).toBeTruthy();
  expect(screen.queryByRole("button", { name: /预览[浅深]色/ })).toBeNull();
  expect(container.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
    "dark",
  );
  fireEvent.click(control);
  expect(options.onPreferencesChange).toHaveBeenCalledExactlyOnceWith({ global_theme: "night" });
});

test("系统主题按受控明暗预览，切换预览不应用主题", () => {
  const options = props();
  const view = render(<BuiltinThemeCard {...options} />);
  expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "预览深色" }));
  expect(options.onTogglePreview).toHaveBeenCalledOnce();
  expect(options.onPreferencesChange).not.toHaveBeenCalled();
  view.rerender(<BuiltinThemeCard {...options} previewTheme="dark" />);
  expect(screen.getByRole("button", { name: "预览浅色" })).toBeTruthy();
  expect(screen.getByText(`${options.entry.title}（深色）`)).toBeTruthy();
});

test.each([false, true])("自定义卡片按目录中的皮肤选中状态显示开关：%s", (packageSelected) => {
  const custom = {
    base: "system" as const,
    candidate_skin: "synthetic-light",
    candidate_skin_dark: "synthetic-dark",
    candidate_colors: { text: "#123456", number: "#abcdef", border: "#fedcba" },
    keyboard: { ...defaultTouchKeyboardSkinDesign, key_radius: 9 },
  };
  const options = props({
    entry: themeEntry("custom"),
    globalTheme: "custom",
    packageSelected,
    preferences: {
      custom_theme: custom,
      candidate_corner_radius: 0,
      candidate_opacity_percent: 80,
    },
    previewCustomTheme: { ...custom, candidate_colors: { text: "#123456" } },
    keyboardDesign: custom.keyboard,
  });
  const { container } = render(<BuiltinThemeCard {...options} />);
  const control = screen.getByRole("switch");
  expect(control.getAttribute("aria-checked")).toBe(String(!packageSelected));
  const preview = container.querySelector<HTMLElement>("[data-skin-preview]")!;
  expect(preview.style.getPropertyValue("--cand-text")).toBe("#123456");
  expect(preview.style.getPropertyValue("--cand-num")).not.toBe("#abcdef");
  expect(preview.style.getPropertyValue("--cand-border")).not.toBe("#fedcba");
  expect(preview.style.getPropertyValue("--msime-skin-radius")).toBe("0px");
  expect(preview.style.getPropertyValue("--msime-candidate-opacity")).toBe("80%");
  fireEvent.click(control);
  expect(options.onPreferencesChange).toHaveBeenCalledExactlyOnceWith({
    global_theme: "custom",
    custom_theme: { ...custom, candidate_skin: null, candidate_skin_dark: null },
  });
  expect(custom.candidate_skin).toBe("synthetic-light");
});

test.each([
  [undefined, "light", true],
  ["night", "dark", false],
  ["paper", "light", false],
] as const)("自定义卡片按基础主题 %s 决定固定明暗", (base, appearance, toggles) => {
  render(
    <BuiltinThemeCard
      {...props({
        entry: themeEntry("custom"),
        preferences: { custom_theme: { base } },
        candidatePreviewTheme: "light",
        previewTheme: "light",
      })}
    />,
  );
  expect(
    screen.getByText(`${themeEntry("custom").title}（${appearance === "dark" ? "深色" : "浅色"}）`),
  ).toBeTruthy();
  expect(Boolean(screen.queryByRole("button", { name: /预览[浅深]色/ }))).toBe(toggles);
});

test.each([
  [false, false, true, false],
  [false, true, false, false],
  [true, false, false, true],
  [true, true, false, true],
])("按宿主选择预览内容 mobile=%s linux=%s", (mobile, linux, toolbar, keyboard) => {
  const options = props({
    entry: themeEntry("custom"),
    mobile,
    linux,
    preferences: { custom_theme: { base: "night" } },
  });
  const { container } = render(<BuiltinThemeCard {...options} />);
  expect(container.querySelectorAll("[data-preview-layout]")).toHaveLength(2);
  expect(Boolean(container.querySelector(".ftb-preview-host"))).toBe(toolbar);
  const preview = container.querySelector("[data-preview-skin]");
  expect(Boolean(preview)).toBe(keyboard);
  if (preview) expect(preview.getAttribute("data-preview-skin")).toBe("night");
});
