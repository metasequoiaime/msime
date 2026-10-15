import { candidateOpacityPercent } from "../candidate/candidate-window-style";
import type { ExternalSkin } from "./external-skins";
import type { PreviewBackground } from "./skin-candidate-preview";

export function dimension(value: number, maximum: number): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= maximum
    ? value
    : 0;
}

export function skinDecorationGeometry(
  skin: Pick<ExternalSkin, "decorationTopDip" | "decorationWidthDip">,
) {
  const top = dimension(skin.decorationTopDip, 500);
  const width = dimension(skin.decorationWidthDip, 1000);
  const decorated = top > 0 && width > 0;
  return { decorated, top: decorated ? top : 0, width: decorated ? width : 0 };
}

export function skinPreviewBackground(
  background: PreviewBackground | undefined,
  opacityPercent?: number,
): PreviewBackground | undefined {
  const opacity = candidateOpacityPercent(opacityPercent) / 100;
  return background && { ...background, opacity: background.opacity * opacity };
}
