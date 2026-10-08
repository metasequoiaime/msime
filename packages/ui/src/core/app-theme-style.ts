import type { CSSProperties } from "react";
import type { AppSeason, ResolvedAppTheme } from "./host-contracts";

/**
 * 让 HarmonyOS 设置按用户应用主题（水杉四季和四个固定季节）重新着色的自定义属性。它们以内联方式写在带 `data-platform` 的元素上，在同一元素上内联声明优先于 `styles.css` 里的 `[data-platform]` 规则，因此所有读取令牌表的后代元素无需第二份样式表就会跟随季节。
 *
 * 两种 HarmonyOS 外观都采用强调色系列。只有手机外观（`harmony`）还采用季节的页面、卡片和细线颜色；2-in-1 外观保留自己的中性表面，与设计稿一致。深色细线沿用令牌表的值，因为解析出的深色 `hair` 是一种共享的灰色，令牌表已与之相符。
 *
 * `--accent-strong` 是共享页面上白色文字下方的填充色。令牌表在两种外观下都把它保持为浅色模式的品牌强调色，以保证白字可读；季节的深色强调色偏浅，所以这里它跟随强调色，并由 `styles.css` 在这两个平台上用 `--p-on-accent` 绘制其上的文字。
 */
export function appThemeStyle(
  theme: ResolvedAppTheme | null,
  dark: boolean,
  platform: "harmony" | "hm2",
): CSSProperties {
  if (!theme) return {};
  const { accent } = theme;
  const variables: Record<string, string> = {
    "--accent-color": accent,
    "--accent-strong": accent,
    "--accent-soft": theme.accent_soft,
    // 令牌表从自己的强调色派生这两个值；改从季节强调色派生，可避免绿色描边或菜单高亮残留在秋季强调色下。
    "--accent-soft-border": `color-mix(in srgb, ${accent} ${dark ? 25 : 22}%, transparent)`,
    "--dropdown-item-hover-bg": `color-mix(in srgb, ${accent} ${dark ? 20 : 12}%, transparent)`,
    "--p-accent-text": accent,
    "--p-on-accent": theme.on_accent,
    "--p-sw-on": accent,
    "--p-btn-fg": accent,
    "--p-seg-on-fg": accent,
  };
  if (platform === "harmony") {
    variables["--p-bg"] = theme.background;
    variables["--p-chrome"] = theme.background;
    variables["--p-group-bg"] = theme.card;
    if (!dark) {
      variables["--p-hair"] = theme.hair;
      variables["--p-row-divider"] = theme.hair;
    }
  }
  return variables as CSSProperties;
}

/** `data-season` 属性的值，以便规则单独选中某一季节；没有解析出主题时不设置。 */
export function seasonAttr(theme: ResolvedAppTheme | null | undefined): AppSeason | undefined {
  return theme?.season;
}
