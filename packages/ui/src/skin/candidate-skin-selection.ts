import type { ExternalSkin } from "./external-skins";
import {
  candidateSkinFor,
  skinDrawsIn,
  skinSlot,
  type CustomTheme,
  type GlobalTheme,
  type ThemeAppearance,
} from "../theme/global-theme";

export type CandidateSkinSelectionPackage = Pick<ExternalSkin, "id" | "name" | "base">;

export interface CandidateSkinSelectionOptions {
  globalTheme: GlobalTheme;
  customTheme?: CustomTheme;
  packages: readonly CandidateSkinSelectionPackage[];
  previewTheme: ThemeAppearance;
}

export interface CandidateSkinSelection {
  skinsInUse: string[];
  /** 当前预览槽位优先，找不到时退到另一已安装槽位；没有对应目录项时为 -1。 */
  packageInUse: number;
  slotSummary: string | null;
  /** 系统底为 null，目录中未知的包为 undefined；应用旧文档时不能混为一类。 */
  slotOf: (id: string) => ThemeAppearance | null | undefined;
}

/** 主题轮播与手机网格共用的皮肤目录选择投影，不改写主题或目录。 */
export function candidateSkinSelection({
  globalTheme,
  customTheme,
  packages,
  previewTheme,
}: CandidateSkinSelectionOptions): CandidateSkinSelection {
  // 两个槽位中的包都算正在使用；同一个包占两槽时保留原来的槽位顺序。
  const skinsInUse =
    globalTheme === "custom"
      ? [customTheme?.candidate_skin, customTheme?.candidate_skin_dark].filter((id): id is string =>
          Boolean(id),
        )
      : [];
  const previewSkin =
    globalTheme === "custom" ? candidateSkinFor(customTheme, previewTheme === "dark") : null;
  const shownSkin = [previewSkin, ...skinsInUse].find((id) =>
    packages.some((skin) => skin.id === id),
  );
  const packageInUse = packages.findIndex((skin) => skin.id === shownSkin);
  const slotOf = (id: string) => {
    const skin = packages.find((entry) => entry.id === id);
    return skin ? skinSlot(skin.base) : undefined;
  };
  // 槽位回退之后仍按包的 base 判断是否绘制；未安装的包按 id 说明。
  const drawnSkinName = (dark: boolean) => {
    const id = candidateSkinFor(customTheme, dark);
    if (!id) return null;
    const skin = packages.find((entry) => entry.id === id);
    if (!skin) return id;
    return skinDrawsIn(skin.base, dark) ? skin.name : null;
  };
  const slotSummary =
    skinsInUse.length > 0
      ? ([false, true] as const)
          .map((dark) => {
            const name = drawnSkinName(dark);
            return `${dark ? "深色" : "浅色"}模式${name ? `用「${name}」` : "不用皮肤"}`;
          })
          .join("，") + "。"
      : null;
  return { skinsInUse, packageInUse, slotSummary, slotOf };
}
