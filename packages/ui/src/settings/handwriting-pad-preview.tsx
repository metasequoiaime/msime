import { useState } from "react";
import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { resolveKeyboardSkinLook } from "../keyboard/keyboard-skin-look";
import type { GlobalTheme } from "../theme/global-theme";
import type { HandwritingStrokeColor } from "./mobile-keyboard-feedback";
import { useTimeout } from "../core/use-timeout";

/** 「水」的四笔，画在 100 × 100 的格子里：竖钩、左边的横撇、右上的撇、右下的捺。 */
const strokes = [
  "M50 12 L50 80 Q50 90 40 85",
  "M20 38 L40 38 L18 68",
  "M76 28 L58 46",
  "M57 47 Q66 66 84 80",
];
const candidates = ["水", "永", "小", "求", "冰"];
/** 一笔写多久、两笔之间停多久。 */
const STROKE_MS = 420;
const STROKE_GAP_MS = 140;
const WRITE_MS = strokes.length * STROKE_MS + (strokes.length - 1) * STROKE_GAP_MS;
/** 候选出来以后停多久再写下一遍。 */
const SHOW_MS = 1600;
/** 固定的笔迹颜色，与鸿蒙键盘的 `HANDWRITING_STROKE_COLORS` 相同；「跟随皮肤」取皮肤的按键文字颜色。 */
const fixedInk: Partial<Record<HandwritingStrokeColor, string>> = {
  black: "#000000",
  white: "#FFFFFF",
  blue: "#2F6FDB",
};
/** 预览里写字的格子边长，单位约等于 vp：笔迹粗细按 vp 画，换算到 100 单位的 viewBox。 */
const GLYPH_SIZE = 120;

type Phase = "writing" | "waiting" | "recognized";

/**
 * 「手写输入」页顶上的手写板预览：用当前皮肤的颜色画键盘上的候选栏和手写板，一笔一笔写出「水」，写完按「识别等待时间」停顿，然后候选栏出现候选、手写板清空，再写下一遍。笔迹的颜色和粗细就是下面两项设置，拖动滑块预览跟着变。
 *
 * 系统设置了减少动态效果时不播动画，直接画写好的字和候选。
 */
export function HandwritingPadPreview({
  theme,
  skin,
  customDesign,
  delayMs,
  strokeColor,
  strokeWidth,
}: {
  theme: "dark" | "light";
  skin: GlobalTheme;
  customDesign?: TouchKeyboardSkinDesign;
  delayMs: number;
  strokeColor: HandwritingStrokeColor;
  strokeWidth: number;
}) {
  const { palette } = resolveKeyboardSkinLook(theme, skin, customDesign);
  const ink = fixedInk[strokeColor] ?? palette.foreground;
  const reducedMotion =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const [phase, setPhase] = useState<Phase>(reducedMotion ? "waiting" : "writing");
  // 每写一遍加一，作为字形的 key，让描笔画的动画从头再播。
  const [round, setRound] = useState(0);

  useTimeout(
    () => {
      if (phase === "writing") {
        setPhase("waiting");
      } else if (phase === "waiting") {
        setPhase("recognized");
      } else {
        setRound((value) => value + 1);
        setPhase("writing");
      }
    },
    reducedMotion ? null : phase === "writing" ? WRITE_MS : phase === "waiting" ? delayMs : SHOW_MS,
  );

  const recognized = phase === "recognized" || reducedMotion;
  return (
    <div
      className="flex flex-col gap-1.5 rounded-[14px] p-2"
      style={{ background: palette.background }}
      data-handwriting-preview={phase}
      aria-hidden="true"
    >
      <div className="flex h-9 items-center gap-5 px-2.5 text-[17px]">
        {recognized &&
          candidates.map((word, index) => (
            <span
              key={word}
              className="animate-ms-fade-in motion-reduce:animate-none"
              style={{ color: index === 0 ? palette.accent : palette.foreground }}
            >
              {word}
            </span>
          ))}
      </div>
      <div
        className="flex h-[150px] items-center justify-center rounded-[10px]"
        style={{ background: palette.key }}
      >
        {(!recognized || reducedMotion) && (
          <svg
            key={round}
            width={GLYPH_SIZE}
            height={GLYPH_SIZE}
            viewBox="0 0 100 100"
            fill="none"
            stroke={ink}
            strokeWidth={(strokeWidth * 100) / GLYPH_SIZE}
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            {strokes.map((d, index) => (
              <path
                key={d}
                d={d}
                pathLength={1}
                className="[stroke-dasharray:1] animate-hand-stroke motion-reduce:animate-none"
                style={{ animationDelay: `${index * (STROKE_MS + STROKE_GAP_MS)}ms` }}
              />
            ))}
          </svg>
        )}
      </div>
    </div>
  );
}
