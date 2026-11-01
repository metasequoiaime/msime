import { themeEntry, customCandidateStyle, themeCandidateStyle } from "../../theme/global-theme";
import { SkinToolbarPreview } from "../../skin/skin-toolbar-preview";
import { SkinPreviewSurface } from "../../skin/skin-preview-surface";
import { useSettingsForm } from "../settings-form-context";
import { Checks, GroupList, LinkRow } from "../../core/platform-controls";
import {
  FloatingToolbarLinuxWindowNotice,
  FloatingToolbarPlatformNotice,
} from "../floating-toolbar-platform-notice";
import { createFloatingToolbarSettingsActions } from "../floating-toolbar-settings-actions";
import { SelectRow } from "../select-row";
import { SwitchRow } from "../switch-row";
import { SliderRow } from "../slider-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsGroupBlock } from "../settings-group-block";
import { SettingsPreviewBlock } from "../settings-preview-block";
import {
  floatingToolbarAutoHideDelay,
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
    showToolbarIdleBehavior,
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
  // Linux 宿主在 X11（以及有 Xwayland 的 Wayland 会话）上画与 macOS 相同的悬浮工具栏，报告 `floating_toolbar_appearance`；页面与 macOS 一样，另在预览下说明画不出来时退回「工具栏」子菜单。没有这项能力的 Linux 宿主只有输入法菜单里的子菜单（IBus 面板属性菜单、Fcitx5 托盘状态区菜单）：页首放说明而不是悬浮条预览，开关和按钮的文案说菜单，尺寸组整组不出现。其他宿主照旧。
  const menuToolbar = linuxPlatform && !showToolbarAppearance;
  // GNOME Shell 下 IBus 只发布输入模式和设置两项，没有「工具栏」子菜单（ClientEngine.cpp 的 publish_mode）。画悬浮条的宿主和只有菜单的宿主都要知道这一点：前者的预览下说明不能说退回子菜单，后者的开关与按钮组的描述不能再指向那个不存在的子菜单。
  const gnomeShell = linuxPlatform && host?.candidate_panel_limit === "gnome_shell";
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
          {linuxPlatform && <FloatingToolbarLinuxWindowNotice gnomeShell={gnomeShell} />}
        </GroupList>
      )}
      <GroupList title="显示">
        <SwitchRow
          title={menuToolbar ? "在输入法菜单显示工具栏" : "在桌面显示悬浮工具栏"}
          description={
            !menuToolbar
              ? "快速访问输入法状态与常用功能"
              : gnomeShell
                ? undefined
                : "「工具栏」子菜单：IBus 在面板的属性菜单里，Fcitx5 在托盘的状态区菜单里，需要桌面提供托盘"
          }
          detail={
            menuToolbar && gnomeShell
              ? "当前 GNOME 桌面的输入源菜单没有「工具栏」子菜单，这个开关在这里不生效"
              : undefined
          }
          checked={floatingToolbar.enabled}
          onChange={(enabled) => onToolbarChange({ enabled })}
        />
        {/* 自动隐藏是悬浮窗的行为，且只有真正读这三个值的宿主才列：Linux 的独立悬浮条或菜单，以及 Windows 与 HarmonyOS 的面板尚未接入这些行为（宿主能力位 floating_toolbar_idle_behavior）。 */}
        {showToolbarIdleBehavior && !menuToolbar && (
          <>
            <SwitchRow
              title="自动隐藏工具栏"
              description="工具栏显示一段时间无交互后淡出隐藏；鼠标悬停不隐藏，切换中英等状态变化时重新显示"
              checked={floatingToolbar.auto_hide}
              onChange={(auto_hide) => onToolbarChange({ auto_hide })}
            />
            <SliderRow
              title="自动隐藏延时"
              description="工具栏显示多久后开始淡出（秒）"
              // 与 Windows 设置页一致：自动隐藏关着时这一行还在，但控件置灰，已选的秒数保留。
              disabled={!floatingToolbar.auto_hide}
              min={1}
              max={60}
              value={floatingToolbarAutoHideDelay(floatingToolbar)}
              valueText={`${floatingToolbarAutoHideDelay(floatingToolbar)} 秒`}
              displayValue={`${floatingToolbarAutoHideDelay(floatingToolbar)} 秒`}
              onChange={(auto_hide_delay) => onToolbarChange({ auto_hide_delay })}
            />
            <SwitchRow
              title="工具栏阴影"
              description="在工具栏周围显示与候选窗一致的柔和阴影"
              checked={floatingToolbar.shadow}
              onChange={(shadow) => onToolbarChange({ shadow })}
            />
          </>
        )}
      </GroupList>
      {showToolbarComponents && (
        <GroupList title="按钮">
          <SettingsGroupBlock>
            <Checks
              legend="按钮"
              description={
                !menuToolbar
                  ? "勾选要显示在悬浮工具栏上的按钮"
                  : gnomeShell
                    ? "勾选工具栏要列出的按钮，当前 GNOME 桌面不生效"
                    : "勾选要显示在「工具栏」子菜单里的按钮"
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
          {/* 只剩菜单的 Linux 宿主没有可调颜色的悬浮条，这条链接和整个「尺寸」组一起不出现。各界面一律跟随颜色模式的宿主（`surfaceThemesFollow`，macOS、Windows、Linux 和 HarmonyOS 2in1）没有工具栏一行，工具栏跟随「主题」页的颜色模式和主题，链接照样指向「主题」页，描述照实说。 */}
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
