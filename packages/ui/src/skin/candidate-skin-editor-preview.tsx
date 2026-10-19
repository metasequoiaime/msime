import { useEffect, useId, useState, type CSSProperties } from "react";
import { customCandidateStyle, skinDrawsIn } from "../theme/global-theme";
import { drawnPackagePalette, skinGeometryStyle } from "./external-skins";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";
import { skinDecorationGeometry } from "./skin-preview-geometry";
import { useSelectedBarPalette } from "./skin-palette";
import { StatusMessage } from "../core/status-message";
import * as settings from "../settings/settings-style";
import {
  draftImageUrl,
  draftSkin,
  fixedMode,
  type CandidateSkinDraft,
  type DraftLayout,
  type DraftMode,
} from "./candidate-skin-draft";

/** 一次「播放动效」依次经过的阶段：先消失，再出现，最后选中第一个候选。 */
export type MotionPhase = "idle" | "exit" | "enter" | "select";

/** 系统要求减少动态效果。预览这时不播放候选窗动效和装饰图待机动效，和各宿主一样。 */
export function usePrefersReducedMotion(): boolean {
  const query = "(prefers-reduced-motion: reduce)";
  const media =
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia(query)
      : null;
  const [reduced, setReduced] = useState(media?.matches ?? false);
  useEffect(() => {
    if (!media) return;
    const update = () => setReduced(media.matches);
    update();
    media.addEventListener?.("change", update);
    return () => media.removeEventListener?.("change", update);
  }, [media]);
  return reduced;
}

/** 按草稿的动效设置依次切换阶段；`play` 从头播放一遍。减少动态效果时什么也不做。 */
export function useMotionReplay(draft: CandidateSkinDraft, reduced: boolean) {
  const [phase, setPhase] = useState<MotionPhase>("idle");
  const [round, setRound] = useState(0);
  const { durationMs, staggerMs } = draft.animation;
  useEffect(() => {
    if (round === 0 || reduced) {
      setPhase("idle");
      return;
    }
    // 出现动画逐行错开，最后一行要多等九个间隔。
    const steps: [MotionPhase, number][] = [
      ["exit", durationMs + 60],
      ["enter", durationMs + staggerMs * 9 + 120],
      ["select", durationMs + 60],
    ];
    const timers: ReturnType<typeof setTimeout>[] = [];
    let elapsed = 0;
    for (const [next, length] of steps) {
      timers.push(setTimeout(() => setPhase(next), elapsed));
      elapsed += length;
    }
    timers.push(setTimeout(() => setPhase("idle"), elapsed));
    return () => timers.forEach(clearTimeout);
  }, [round, reduced, durationMs, staggerMs]);
  return { phase, play: () => setRound((value) => value + 1) };
}

/**
 * 编辑器的实时预览：一种明暗下的候选窗，用与本地皮肤卡片相同的 `SkinCandidatePreview` 和同样的配色、几何层次（底色主题、包的配色、包的几何）画出草稿。
 *
 * 不叠加用户在「候选窗口」页设的圆角和不透明度，预览的是皮肤本身。动效按 web 引擎（`packages/web-engine/src/candidates.js`）的同名关键帧播放，样式在 `styles.css` 的 `skin-editor-motion`。
 */
export function CandidateSkinModePreview({
  draft,
  mode,
  orientation,
  phase,
  reducedMotion,
}: {
  draft: CandidateSkinDraft;
  mode: DraftMode;
  orientation: DraftLayout;
  phase: MotionPhase;
  reducedMotion: boolean;
}) {
  const scope = `skin-editor-preview-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const skin = draftSkin(draft);
  const dark = mode === "dark";
  const fixed = fixedMode(draft.base);
  const drawn = skinDrawsIn(skin.base, dark) && (fixed !== null || draft.modes.includes(mode));
  const palette = drawnPackagePalette(skin, mode);
  const paletteFailed = useSelectedBarPalette(scope, drawn && palette?.showSelectedBar === false);
  const title = dark ? "深色" : "浅色";
  if (!drawn)
    return (
      <div className={settings.skinEditorPreviewEmpty} role="group" aria-label={`${title}预览`}>
        <span className={settings.skinEditorPreviewLabel}>{title}</span>
        <StatusMessage className={settings.skinCardDescription}>
          {fixed
            ? `底色是${fixed === "dark" ? "深色" : "浅色"}主题，这款皮肤只用在${fixed === "dark" ? "深色" : "浅色"}模式。`
            : `没有勾选${title}模式，${title}模式下不使用这款皮肤。`}
        </StatusMessage>
      </div>
    );
  const { decorated } = skinDecorationGeometry(skin);
  const decoration = draft.decoration;
  const background = draft.background;
  const geometry = {
    ...customCandidateStyle(skin.base, undefined, palette),
    ...skinGeometryStyle(skin, mode),
    "--msime-editor-duration": `${draft.animation.durationMs}ms`,
    "--msime-editor-stagger": `${draft.animation.staggerMs}ms`,
    "--msime-editor-period": `${decoration?.motionPeriodMs ?? 3000}ms`,
  } as CSSProperties;
  const motion = draft.animation.enabled && !reducedMotion;
  return (
    <div
      role="group"
      aria-label={`${title}预览`}
      className={`${settings.skinEditorPreviewCard}${decorated ? " external-skin-decorated" : ""}`}
    >
      <span className={settings.skinEditorPreviewLabel}>{title}</span>
      <SkinPreviewSurface
        className={`skin-editor-motion ${scope}${mode === "light" ? " theme-light" : ""}`}
        style={geometry}
        data-preview-theme={mode}
        data-decoration-align={decoration?.align ?? "right"}
        data-decoration-motion={reducedMotion ? "none" : (decoration?.motion ?? "none")}
        data-motion-phase={motion ? phase : "idle"}
        data-motion-enter={draft.animation.enter}
        data-motion-exit={draft.animation.exit}
        data-motion-selection={draft.animation.selection}
        aria-hidden="true"
      >
        <SkinPreviewStage>
          <SkinCandidatePreview
            orientation={orientation}
            decorated={decorated}
            image={decoration ? draftImageUrl(decoration.image) : undefined}
            background={
              background
                ? {
                    url: draftImageUrl(background.image),
                    fit: background.fit,
                    opacity: background.opacity,
                  }
                : undefined
            }
            count={orientation === "horizontal" ? 5 : 6}
          />
        </SkinPreviewStage>
      </SkinPreviewSurface>
      {paletteFailed && (
        <StatusMessage role="status" className={settings.skinCardDescription}>
          当前浏览器无法隐藏选中竖条，其余配色照常预览。
        </StatusMessage>
      )}
    </div>
  );
}
