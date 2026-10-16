import { themeEntry, customCandidateStyle, themeCandidateStyle } from "../../theme/global-theme";
import { SkinToolbarPreview } from "../../skin/skin-toolbar-preview";
import { SkinPreviewSurface } from "../../skin/skin-preview-surface";
import { useSettingsForm } from "../settings-form-context";
import { Checks, GroupList, LinkRow } from "../../core/platform-controls";
import { FloatingToolbarPlatformNotice } from "../floating-toolbar-platform-notice";
import { createFloatingToolbarSettingsActions } from "../floating-toolbar-settings-actions";
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsGroupBlock } from "../settings-group-block";
import { SettingsPreviewBlock } from "../settings-preview-block";
import {
  floatingToolbarButtonOptions,
  floatingToolbarSize,
  floatingToolbarSizes,
} from "../floating-toolbar-options";

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
    previewCustomTheme,
    linuxPlatform,
    selectPage,
    surfaceThemesFollow,
  } = useSettingsForm();
  const { onChange: onToolbarChange } = createFloatingToolbarSettingsActions({
    setDraft,
  });
  // Linux 两个前端都不画悬浮窗，工具栏是输入法菜单里的子菜单（IBus 面板属性菜单、Fcitx5 托盘状态区菜单）。页面按这个呈现：页首放说明而不是悬浮条预览，开关和按钮的文案说菜单，尺寸组整组不出现。其他宿主照旧。
  const menuToolbar = linuxPlatform;
  // GNOME Shell 下 IBus 只发布输入模式和设置两项，没有「工具栏」子菜单（ClientEngine.cpp 的 publish_mode）。判据和页首说明同一个，开关与按钮组的描述不能再指向那个不存在的子菜单。
  const gnomeShell = menuToolbar && host?.candidate_panel_limit === "gnome_shell";
  // 缩放和图标尺寸单独调过、凑不成任何一档时显示「自定义」，打开页面不改写存下的值。
  const toolbarSize = floatingToolbarSize(floatingToolbar);
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
                      previewCustomTheme?.base,
                      previewCustomTheme?.candidate_colors,
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
              items={floatingToolbarButtonOptions(floatingToolbar, host)}
              onChange={(key, checked) => onToolbarChange({ [key]: checked })}
            />
          </SettingsGroupBlock>
        </GroupList>
      )}
      {!menuToolbar && (
        <GroupList title="尺寸">
          {/* 缩放和图标尺寸相乘才是工具栏的大小，页面只给一个「工具栏大小」，选一档同时写两个值（见 `floatingToolbarSizes`）。 */}
          {showToolbarAppearance && (
            <SelectRow
              title="工具栏大小"
              description="按钮和图标一起放大或缩小，不改变系统显示缩放"
              value={toolbarSize ? String(toolbarSize.percent) : "custom"}
              onChange={(event) => {
                const size = floatingToolbarSizes.find(
                  (item) => String(item.percent) === event.target.value,
                );
                if (size)
                  onToolbarChange({ scale_percent: size.scale_percent, font_size: size.font_size });
              }}
            >
              {!toolbarSize && (
                <option value="custom" disabled>
                  {`自定义（缩放 ${floatingToolbar.scale_percent}%，图标 ${floatingToolbar.font_size}）`}
                </option>
              )}
              {floatingToolbarSizes.map((size) => (
                <option key={size.percent} value={size.percent}>
                  {size.percent}%
                </option>
              ))}
            </SelectRow>
          )}
          {/* Linux 宿主不读 toolbar_theme，「主题」页没有工具栏一行可指，所以这条链接和整个「尺寸」组一起只在非 Linux 宿主上出现。各界面一律跟随颜色模式的宿主（`surfaceThemesFollow`，macOS、Windows 和 HarmonyOS 2in1）也没有工具栏一行，工具栏跟随「主题」页的颜色模式和主题，链接照样指向「主题」页，描述照实说。 */}
          <LinkRow
            title="颜色与明暗"
            description={
              surfaceThemesFollow
                ? "悬浮工具栏跟随「主题」页的颜色模式和主题"
                : "悬浮工具栏的明暗在「主题」页设置"
            }
            onClick={() => selectPage("skin")}
          />
        </GroupList>
      )}
    </SettingsPageFieldset>
  );
}
