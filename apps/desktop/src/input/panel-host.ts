import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SettingsClient } from "../../../../packages/ui/src/core/settings-client";
import type { SurfaceTheme } from "../../../../packages/ui/src/settings/theme-preferences";

/** 面板窗口需要的宿主能力子集。设置客户端已有能力时优先复用那份数据。 */
export type PanelHost = {
  platform: string;
  touch_keyboard_spacing?: boolean;
  surface_themes_follow?: boolean;
};

/** `resolved` 为 false 时仍在等待外壳报告能力；没有能力或不在 Tauri 中也算已解析。 */
export type PanelHostState = { host?: PanelHost; resolved: boolean };

/** 读取面板窗口的宿主能力，避免各个面板分别实现同一套回退与生命周期。 */
export function usePanelHost(host: SettingsClient["host"]): PanelHostState {
  const [state, setState] = useState<PanelHostState>({
    host,
    resolved: Boolean(host?.platform),
  });
  useEffect(() => {
    let active = true;
    if (host?.platform)
      setState((current) =>
        current.host === host && current.resolved ? current : { host, resolved: true },
      );
    else if (isTauri())
      void invoke<PanelHost>("host_capabilities")
        .then((value) => {
          if (active) setState({ host: value, resolved: true });
        })
        .catch(() => {
          if (active) setState({ host: undefined, resolved: true });
        });
    else
      setState((current) =>
        current.host === undefined && current.resolved
          ? current
          : { host: undefined, resolved: true },
      );
    return () => {
      active = false;
    };
  }, [host]);
  return state;
}

/** 返回面板实际使用的宿主平台。 */
export function useHostPlatform(host: SettingsClient["host"]): string | undefined {
  return usePanelHost(host).host?.platform;
}

/** 宿主要求所有面板跟随颜色模式时，忽略已保存的面板专属主题。 */
export function panelSurfaceTheme(
  { host, resolved }: PanelHostState,
  surface: SurfaceTheme | undefined,
): SurfaceTheme | undefined {
  return !resolved || host?.surface_themes_follow === true ? undefined : surface;
}
