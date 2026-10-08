import { useEffect, useState } from "react";
import type { AppThemeClient, HostChromeClient, ResolvedAppTheme } from "../core/host-contracts";
import { mixKeyboardColor } from "../keyboard/keyboard-colors";
import { brandAccent } from "../theme/platform-tokens";
import { resolveSettingsTheme } from "./theme-helpers";
import type { SurfaceTheme, ThemeMode } from "./theme-settings-section";

/** 同步设置页文档的主题，需要时跟随系统配色。返回设置页当前是否以深色绘制，这样宿主按外观解析的颜色（鸿蒙的应用主题、系统栏）能与文档采用同一个答案。 */
export function useSettingsTheme(themeMode: ThemeMode, settingsTheme: SurfaceTheme): boolean {
  const [resolved, setResolved] = useState(() => resolveSettingsTheme(themeMode, settingsTheme));
  useEffect(() => {
    if (typeof document === "undefined") return;
    const apply = () => {
      const next = resolveSettingsTheme(themeMode, settingsTheme);
      document.documentElement.dataset.theme = next;
      setResolved(next);
    };
    apply();
    if (
      themeMode !== "system" ||
      settingsTheme !== "follow" ||
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    )
      return;
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const listener = () => apply();
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", listener);
      return () => media.removeEventListener("change", listener);
    }
    media.addListener(listener);
    return () => media.removeListener(listener);
  }, [settingsTheme, themeMode]);
  return resolved === "dark";
}

/** 按当前外观解析的宿主应用主题；外观变化、宿主报告新的主题或季节时重新解析。没有客户端或宿主暂时无法解析时为 null。 */
export function useResolvedAppTheme(
  client: AppThemeClient | undefined,
  dark: boolean,
): ResolvedAppTheme | null {
  // 首次渲染时也解析，页面不会先以平台默认色画一帧再换成季节色。
  const [theme, setTheme] = useState(() => client?.resolve(dark) ?? null);
  useEffect(() => {
    if (!client) {
      setTheme(null);
      return;
    }
    const refresh = () => setTheme(client.resolve(dark));
    refresh();
    return client.subscribe(refresh);
  }, [client, dark]);
  return theme;
}

/** 鸿蒙手机的状态栏取页面背景（应用主题下是季节的背景），导航栏取标签栏的固定颜色，使系统界面在两种外观下都像页面的一部分。 */
export function useHarmonySystemBars(
  chrome: HostChromeClient | undefined,
  theme: ResolvedAppTheme | null,
  dark: boolean,
): void {
  const background = theme?.background ?? (dark ? "#000000" : "#F1F3F5");
  useEffect(() => {
    chrome?.setSystemBars({
      background,
      navigationBar: dark ? "#141414" : "#F1F3F5",
      dark,
    });
  }, [chrome, background, dark]);
}

/**
 * 欢迎流程显示期间鸿蒙手机的系统栏；设置页的 `useHarmonySystemBars` 管不到这里，因为那时页面还没挂载。
 *
 * 开屏动画播放时两条栏都取它的底色（#0A0B0A 上叠 20% 的强调色，与 `hSplashShell` 画的一致）并用浅色内容，让开屏从边到边铺满屏幕。之后的步骤两条栏都取页面颜色，因为流程没有标签栏：应用主题下是季节的背景，否则是平台的背景。
 */
export function useHarmonyWelcomeSystemBars(
  chrome: HostChromeClient | undefined,
  theme: ResolvedAppTheme | null,
  dark: boolean,
  splashing: boolean,
): void {
  const accent = theme?.accent ?? (dark ? brandAccent.dark : brandAccent.light);
  const background = splashing
    ? mixKeyboardColor("#0A0B0A", accent, 0.2)
    : (theme?.background ?? (dark ? "#000000" : "#F1F3F5"));
  const barsDark = splashing || dark;
  useEffect(() => {
    chrome?.setSystemBars({ background, navigationBar: background, dark: barsDark });
  }, [chrome, background, barsDark]);
}
