import { useEffect, useId, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import type { Preferences } from "../index";
import {
  decorationImage,
  dimension,
  drawnPackagePalette,
  skinGeometryStyle,
  useSkinPreviewAssets,
  type ExternalSkin,
  type SkinCatalog,
} from "./external-skins";
import { useSelectedBarPalette } from "./skin-palette";
import {
  candidatePaletteStyle,
  candidateSkinFor,
  customCandidatePalette,
  customDrawnBase,
  skinDrawsIn,
  themeEntry,
  type ResolvedTheme,
  type ResolveThemeRequest,
} from "../theme/global-theme";
import type { SkinImageReader } from "./skin-image";
import { ReservedCandidatePreview, type PreviewReserve } from "./skin-candidate-preview";
import { candidateFontSize, candidateFontStyle } from "../candidate/candidate-font-size";
import { candidateFamilyStyle } from "../candidate/candidate-font-family";
import { candidateOpacityPercent, candidateWindowStyle } from "../candidate/candidate-window-style";
import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";

/** The host's own `resolve()` answer for `request`, when the host has a theme call; `undefined` until it arrives, when it fails, and for a request it was not asked for. */
function useResolvedTheme(
  resolve: ((request: ResolveThemeRequest) => Promise<ResolvedTheme>) | undefined,
  request: ResolveThemeRequest,
): ResolvedTheme | undefined {
  const key = JSON.stringify(request);
  const [result, setResult] = useState<{ key: string; theme: ResolvedTheme }>();
  useEffect(() => {
    if (!resolve) return;
    let current = true;
    resolve(JSON.parse(key) as ResolveThemeRequest).then(
      (theme) => {
        if (current) setResult({ key, theme });
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, [resolve, key]);
  return resolve && result?.key === key ? result.theme : undefined;
}

function LoadedPreview({
  skin,
  preferences,
  readImage,
  resolve,
  helpcode,
  theme,
  reserve,
}: {
  skin: ExternalSkin;
  preferences: Preferences;
  readImage?: SkinImageReader;
  resolve?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  helpcode: boolean;
  /** 宿主当前的明暗模式，也就是 `resolve()` 的 `dark`。 */
  theme: "dark" | "light";
  reserve?: PreviewReserve;
}) {
  const scope = `appearance-external-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const layout = preferences.candidate_layout ?? "vertical";
  const dark = theme === "dark";
  // The host's `resolve()` is the authority; until it answers, and on hosts without it, the page's mirror layers the base, the package palette for the drawn mode (none when the package does not declare it) and the pickers the same way.
  const resolved = useResolvedTheme(resolve, {
    global_theme: "custom",
    custom_theme: preferences.custom_theme,
    dark,
    layout,
  });
  // 与 `resolve()` 同一套规则：包只在它 base 的明暗下画，画了它时底是包的 base，否则按 `customDrawnBase`；固定明暗的底决定取哪种配色。
  const drawsIn = skinDrawsIn(skin.base, dark);
  const base = customDrawnBase(preferences.custom_theme, drawsIn ? skin.base : null, dark);
  const mode = themeEntry(base).appearance ?? theme;
  const palette = resolved
    ? resolved.candidate
    : customCandidatePalette(
        base,
        preferences.custom_theme?.candidate_colors,
        drawsIn ? drawnPackagePalette(skin, mode) : null,
      );
  const hideBar = palette?.show_selected_bar === false;
  const paletteFailed = useSelectedBarPalette(scope, hideBar);
  const decorated =
    dimension(skin.decorationTopDip, 500) > 0 && dimension(skin.decorationWidthDip, 1000) > 0;
  const decoration = decorated ? decorationImage(skin) : null;
  const { image, decodeFailed, onImageError, background } = useSkinPreviewAssets(
    readImage,
    skin,
    decoration,
    0,
  );
  const geometry = {
    ...candidatePaletteStyle(palette),
    ...candidateFontStyle(preferences),
    ...candidateFamilyStyle(preferences),
    ...skinGeometryStyle(skin, mode),
    // After the package geometry: the user's corner radius beats the package's, as on the hosts.
    ...candidateWindowStyle(preferences),
  } as CSSProperties;
  // The window opacity fades the package background along with the surface it lies on.
  const opacity = candidateOpacityPercent(preferences.candidate_opacity_percent) / 100;
  const drawnBackground = background.drawn && {
    ...background.drawn,
    opacity: background.drawn.opacity * opacity,
  };
  return (
    <div className={decorated ? "external-skin-decorated" : undefined}>
      <SkinPreviewSurface
        className={`appearance-candidate-preview ${scope}`}
        style={geometry}
        data-preview-theme={mode}
        data-decoration-align={skin.decorationAlign ?? "right"}
        data-font-size={candidateFontSize(preferences.candidate_font_size)}
        aria-hidden="true"
      >
        <SkinPreviewStage>
          <ReservedCandidatePreview
            reserve={reserve}
            orientation={layout}
            count={preferences.candidate_page_size}
            preedit={preferences.candidate_preedit_style !== "empty"}
            helpcode={helpcode}
            decorated={decorated}
            image={decodeFailed ? undefined : image?.url}
            onImageError={onImageError}
            background={drawnBackground}
            onBackgroundError={background.onError}
          />
        </SkinPreviewStage>
      </SkinPreviewSurface>
      {paletteFailed && (
        <StatusMessage role="status">
          当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。
        </StatusMessage>
      )}
      {(image?.failed || decodeFailed || background.failed) && (
        <StatusMessage role="status">
          皮肤图片加载失败，保留基础预览。可刷新预览重试。
        </StatusMessage>
      )}
      {(decoration || skin.background) && !readImage && (
        <StatusMessage role="status">当前宿主不支持皮肤图片预览。</StatusMessage>
      )}
    </div>
  );
}

export function ExternalAppearancePreview({
  preferences,
  scan,
  readImage,
  resolve,
  active,
  revision,
  helpcode,
  theme,
  plain,
  reserve,
}: {
  preferences: Preferences;
  scan?: () => Promise<SkinCatalog>;
  readImage?: SkinImageReader;
  /** `SettingsClient.resolveTheme`, when the host has it. */
  resolve?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  active: boolean;
  revision: number;
  helpcode: boolean;
  /** 宿主当前的明暗模式：决定取哪个槽位的皮肤，也就是 `resolve()` 的 `dark`。 */
  theme: "dark" | "light";
  /** 不带皮肤包的预览（底加取色器）。所选皮肤不在这种明暗下画时画它，与 `resolve()` 一致。 */
  plain: ReactNode;
  /** 见 `ReservedCandidatePreview`。 */
  reserve?: PreviewReserve;
}) {
  const [refresh, setRefresh] = useState(0);
  const dark = theme === "dark";
  const id = candidateSkinFor(preferences.custom_theme, dark);
  const key = useMemo(() => ({}), [scan, id, active, revision, refresh]);
  const [result, setResult] = useState<{ key: object; skin?: ExternalSkin; failed?: boolean }>();
  useEffect(() => {
    let current = true;
    if (active && scan)
      void (async () => {
        try {
          const catalog = await scan();
          if (current) setResult({ key, skin: catalog.packages.find((skin) => skin.id === id) });
        } catch {
          if (current) setResult({ key, failed: true });
        }
      })();
    return () => {
      current = false;
    };
  }, [key, active, scan, id]);
  if (!active) return null;
  if (!scan)
    return (
      <StatusMessage role="status">当前宿主不支持扫描外部皮肤，无法预览所选皮肤。</StatusMessage>
    );
  const current = result?.key === key ? result : undefined;
  const skin = current?.skin;
  // 固定明暗底的包只在那种明暗下画，`resolve()` 在别的模式下画底和取色器，预览照样画不带包的那个；`system` 底的包按当前模式取配色，仍要声明这个模式。
  const fixed = skin ? themeEntry(skin.base).appearance : null;
  const drawsIn = skin ? skinDrawsIn(skin.base, dark) : false;
  const compatible =
    skin?.layouts.includes(preferences.candidate_layout ?? "vertical") &&
    (fixed !== null || skin.themes.includes(theme));
  return (
    <>
      <ActionButton
        action={() => setRefresh((value) => value + 1)}
        className="skin-preview-switch"
        disabled={!current}
        label="刷新预览"
      />
      {!current ? (
        <StatusMessage role="status">正在读取所选皮肤。</StatusMessage>
      ) : current.failed ? (
        <StatusMessage role="status">读取所选皮肤失败，请刷新预览重试。</StatusMessage>
      ) : !skin ? (
        <StatusMessage role="status">未找到所选皮肤，请检查皮肤目录后刷新预览。</StatusMessage>
      ) : !drawsIn ? (
        <>
          <StatusMessage role="status">
            所选皮肤是{fixed === "dark" ? "深色" : "浅色"}皮肤，
            {theme === "dark" ? "深色" : "浅色"}模式下不使用它。
          </StatusMessage>
          {plain}
        </>
      ) : !compatible ? (
        <StatusMessage role="status">
          所选皮肤不支持当前布局或{theme === "dark" ? "深色" : "浅色"}模式。
        </StatusMessage>
      ) : (
        <LoadedPreview
          skin={skin}
          preferences={preferences}
          readImage={readImage}
          resolve={resolve}
          helpcode={helpcode}
          theme={theme}
          reserve={reserve}
        />
      )}
    </>
  );
}
