import * as settings from "../settings-style";
import { themeEntry, customCandidateStyle, themeCandidateStyle } from "../../theme/global-theme";
import { SkinToolbarPreview } from "../../skin/skin-toolbar-preview";
import { SkinPreviewSurface } from "../../skin/skin-preview-surface";
import type { FloatingToolbarPreferences, HostCapabilities } from "../../index";
import { useSettingsForm } from "../settings-form-context";
import { Checks, GroupList, LinkRow } from "../../core/platform-controls";
import { FloatingToolbarPlatformNotice } from "../floating-toolbar-platform-notice";
import { createFloatingToolbarSettingsActions } from "../floating-toolbar-settings-actions";
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsGroupBlock } from "../settings-group-block";
import { SettingsPreviewBlock } from "../settings-preview-block";

type FloatingToolbarOptionKey = keyof Pick<
  FloatingToolbarPreferences,
  | "english_mode"
  | "input_scheme"
  | "fullwidth"
  | "punctuation"
  | "character_set"
  | "emoji"
  | "handwriting"
  | "screen_keyboard"
  | "voice"
  | "settings"
>;
/// In the order the buttons sit on the toolbar. The third entry names the capability a host must
/// report for the switch to be offered at all: the handwriting and voice buttons are this client's
/// own additions and only one host draws them, so a switch for them elsewhere would turn off
/// something that is not there.
const floatingToolbarOptions: [
  FloatingToolbarOptionKey,
  string,
  (
    | keyof Pick<
        HostCapabilities,
        "floating_toolbar_handwriting" | "floating_toolbar_voice" | "floating_toolbar_input_scheme"
      >
    | null
  ),
][] = [
  ["english_mode", "英文输入模式", null],
  ["input_scheme", "切换输入方案", "floating_toolbar_input_scheme"],
  ["fullwidth", "全角 / 半角", null],
  ["punctuation", "中英文标点", null],
  ["character_set", "简繁切换", null],
  ["emoji", "表情与符号", null],
  ["handwriting", "手写识别板", "floating_toolbar_handwriting"],
  ["screen_keyboard", "屏幕键盘", null],
  ["voice", "语音输入", "floating_toolbar_voice"],
  ["settings", "设置", null],
];
const floatingToolbarScales: FloatingToolbarPreferences["scale_percent"][] = [75, 100, 125, 150];
const floatingToolbarFontSizes: FloatingToolbarPreferences["font_size"][] = [
  16, 18, 20, 22, 24, 26, 28,
];

/** The 悬浮工具栏 page of the settings form. */
export function FloatingToolbarSettingsPage() {
  const {
    host,
    showToolbarAppearance,
    showToolbarComponents,
    draft,
    setDraft,
    busy,
    page,
    floatingToolbar,
    toolbarPreviewTheme,
    globalTheme,
    linuxPlatform,
    selectPage,
  } = useSettingsForm();
  const { onChange: onToolbarChange } = createFloatingToolbarSettingsActions({
    setDraft,
  });
  // Linux 两个前端都不画悬浮窗，工具栏是输入法菜单里的子菜单（IBus 面板属性菜单、Fcitx5 托盘状态区菜单）。页面按这个呈现：页首放说明而不是悬浮条预览，开关和按钮的文案说菜单，尺寸组整组不出现。其他宿主照旧。
  const menuToolbar = linuxPlatform;
  // GNOME Shell 下 IBus 只发布输入模式和设置两项，没有「工具栏」子菜单（ClientEngine.cpp 的 publish_mode）。判据和页首说明同一个，开关与按钮组的描述不能再指向那个不存在的子菜单。
  const gnomeShell = menuToolbar && host?.candidate_panel_limit === "gnome_shell";
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "floating-toolbar"}
      ariaLabel="悬浮工具栏"
    >
      {/* 预览放在页首：下面每一组改的都是它画出的内容。Linux 没有可预览的悬浮条，页首换成说明。 */}
      {menuToolbar ? (
        <GroupList>
          <FloatingToolbarPlatformNotice buttons={showToolbarComponents} gnomeShell={gnomeShell} />
        </GroupList>
      ) : (
        <GroupList>
          <SettingsPreviewBlock aria-label="悬浮工具栏预览">
            <SkinPreviewSurface
              data-toolbar-preview=""
              data-global-theme={globalTheme}
              data-preview-theme={
                themeEntry(
                  globalTheme === "custom" ? (draft.custom_theme?.base ?? "system") : globalTheme,
                ).appearance ?? toolbarPreviewTheme
              }
              style={
                globalTheme === "custom"
                  ? customCandidateStyle(
                      draft.custom_theme?.base,
                      draft.custom_theme?.candidate_colors,
                    )
                  : themeCandidateStyle(globalTheme)
              }
            >
              <SkinToolbarPreview preferences={floatingToolbar} />
            </SkinPreviewSurface>
          </SettingsPreviewBlock>
        </GroupList>
      )}
      <GroupList title="显示">
        <SwitchRow
          title={menuToolbar ? "在输入法菜单显示工具栏" : "在桌面显示悬浮工具栏"}
          description={
            gnomeShell
              ? "当前 GNOME 桌面的输入源菜单没有「工具栏」子菜单，这个开关在这里不生效"
              : menuToolbar
                ? "「工具栏」子菜单：IBus 在面板的属性菜单里，Fcitx5 在托盘的状态区菜单里，需要桌面提供托盘"
                : "快速访问输入法状态与常用功能"
          }
          checked={floatingToolbar.enabled}
          onChange={(enabled) => onToolbarChange({ enabled })}
        />
      </GroupList>
      {showToolbarComponents && (
        <GroupList title="按钮">
          <SettingsGroupBlock>
            <Checks
              legend="按钮"
              description={
                gnomeShell
                  ? "勾选工具栏要列出的按钮，当前 GNOME 桌面不生效"
                  : menuToolbar
                    ? "勾选要显示在「工具栏」子菜单里的按钮"
                    : "勾选要显示在悬浮工具栏上的按钮"
              }
              items={[
                // The mode switch is the toolbar's reason to exist, so its box is drawn checked and cannot be cleared.
                {
                  value: "mode_switch" as const,
                  label: (
                    <>
                      中英文切换
                      <span className={settings.toolbarRequiredLabel}>始终显示</span>
                    </>
                  ),
                  checked: true,
                  disabled: true,
                },
                ...floatingToolbarOptions
                  .filter(([, , capability]) => !capability || !host || host[capability])
                  .map(([key, label]) => ({
                    value: key,
                    label,
                    checked: floatingToolbar[key],
                  })),
              ]}
              onChange={(key, checked) => {
                if (key !== "mode_switch") onToolbarChange({ [key]: checked });
              }}
            />
          </SettingsGroupBlock>
        </GroupList>
      )}
      {!menuToolbar && (
        <GroupList title="尺寸">
          {showToolbarAppearance && (
            <>
              <SelectRow
                title="工具栏缩放"
                description="相对系统 DPI 的额外缩放，不改变系统显示缩放"
                value={floatingToolbar.scale_percent}
                onChange={(event) =>
                  onToolbarChange({
                    scale_percent: Number(
                      event.target.value,
                    ) as FloatingToolbarPreferences["scale_percent"],
                  })
                }
              >
                {floatingToolbarScales.map((value) => (
                  <option key={value} value={value}>
                    {value}%
                  </option>
                ))}
              </SelectRow>
              <SelectRow
                title="图标尺寸"
                description="图标基准大小（像素），再乘以上方缩放"
                value={floatingToolbar.font_size}
                onChange={(event) =>
                  onToolbarChange({
                    font_size: Number(
                      event.target.value,
                    ) as FloatingToolbarPreferences["font_size"],
                  })
                }
              >
                {floatingToolbarFontSizes.map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </SelectRow>
            </>
          )}
          {/* Linux 宿主不读 toolbar_theme，「主题」页没有工具栏一行可指，所以这条链接和整个「尺寸」组一起只在非 Linux 宿主上出现。 */}
          <LinkRow
            title="颜色与明暗"
            description="悬浮工具栏的明暗在「主题」页设置"
            onClick={() => selectPage("skin")}
          />
        </GroupList>
      )}
    </SettingsPageFieldset>
  );
}
