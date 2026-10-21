import { useLayoutEffect, useMemo, useRef } from "react";
import { decorationCandidate, type DecorationCandidate } from "./decoration-candidate";

/** 只绘制预览样例；样式与图片生命周期都归当前预览节点。 */
export function SkinDecorationWord({
  candidate,
  samples,
  image,
}: {
  candidate?: DecorationCandidate | null;
  samples: readonly (readonly [string, string])[];
  image: string;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const style = useMemo(() => decorationCandidate(candidate), [candidate]);
  const index = style?.index === 0 ? 0 : (style?.index ?? 0) - 1;
  const text = samples[index]?.[0] ?? "";
  useLayoutEffect(() => {
    const word = ref.current;
    const parent = word?.parentElement;
    const picture = parent?.querySelector<HTMLImageElement>(".skin-decoration-image");
    if (!word || !parent || !picture || !style || !text) return;
    const layout = () => {
      word.hidden = !picture.complete || picture.naturalWidth === 0;
      if (word.hidden) return;
      const geometry = getComputedStyle(parent);
      const width = parseFloat(geometry.getPropertyValue("--msime-skin-decoration-width"));
      const height = parseFloat(geometry.getPropertyValue("--msime-skin-decoration-top"));
      if (!(width > 0 && height > 0)) {
        word.hidden = true;
        return;
      }
      word.style.color = style.resolvedColor ?? "#FFFFFF";
      word.style.fontSize = `${style.fontSizeDip}px`;
      const natural = word.offsetWidth;
      const limit = style.maxWidthDip > 0 ? style.maxWidthDip : width;
      if (natural > limit && natural > 0) {
        word.style.fontSize = `${Math.max(Math.min(style.fontSizeDip, style.minFontSizeDip), (style.fontSizeDip * limit) / natural)}px`;
      }
      const left =
        style.alignH === "left"
          ? 0
          : (width - word.offsetWidth) * (style.alignH === "center" ? 0.5 : 1);
      const top =
        style.alignV === "top"
          ? 0
          : (height - word.offsetHeight) * (style.alignV === "center" ? 0.5 : 1);
      word.style.left = `${picture.offsetLeft + left + style.offsetXDip}px`;
      word.style.top = `${picture.offsetTop + picture.offsetHeight - height + top + style.offsetYDip}px`;
    };
    const failed = () => {
      word.hidden = true;
    };
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(layout);
    observer?.observe(parent);
    observer?.observe(picture);
    picture.addEventListener("load", layout);
    picture.addEventListener("error", failed);
    document.fonts?.addEventListener("loadingdone", layout);
    layout();
    return () => {
      observer?.disconnect();
      picture.removeEventListener("load", layout);
      picture.removeEventListener("error", failed);
      document.fonts?.removeEventListener("loadingdone", layout);
    };
  }, [style, image, text]);
  return style && text ? (
    <span className="skin-decoration-word" ref={ref} hidden aria-hidden="true">
      {text}
    </span>
  ) : null;
}
