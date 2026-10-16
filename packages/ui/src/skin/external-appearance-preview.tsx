import { useEffect, useId, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import { preeditControlState } from "../settings/preedit-preferences";
import {
  decorationImage,
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
import { candidateWindowStyle } from "../candidate/candidate-window-style";
import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";
import { skinDecorationGeometry, skinPreviewBackground } from "./skin-preview-geometry";
import { useAsyncGeneration } from "../settings/use-async-generation";

/** 使用宿主的主题解析；同一皮肤支持横竖两种排布时，等待新排布的结果期间保留已有配色。 */
function useResolvedTheme(
  resolve: ((request: ResolveThemeRequest) => Promise<ResolvedTheme>) | undefined,
  request: ResolveThemeRequest,
): ResolvedTheme | undefined {
  const key = JSON.stringify(request);
  const paletteKey = JSON.stringify({ ...request, layout: undefined });
  const [result, setResult] = useState<{
    paletteKey: string;
    resolve: typeof resolve;
    theme: ResolvedTheme;
  }>();
  const generation = useAsyncGeneration(resolve, key);
  useEffect(() => {
    if (!resolve) return;
    const current = generation.current;
    resolve(JSON.parse(key) as ResolveThemeRequest).then(
      (theme) => {
        if (generation.current === current) setResult({ paletteKey, resolve, theme });
      },
      () => {},
    );
  }, [resolve, key, paletteKey, generation]);
  return resolve && result?.resolve === resolve && result.paletteKey === paletteKey
    ? result.theme
    : undefined;
}

function LoadedPreview({
  skin,
  preferences,
  readImage,
  resolve,
  helpcode,
  brand,
  theme,
  reserve,
}: {
  skin: ExternalSkin;
  preferences: Preferences;
  readImage?: SkinImageReader;
  resolve?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  helpcode: boolean;
  brand?: boolean;
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
  const { decorated } = skinDecorationGeometry(skin);
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
  // 窗口不透明度连同包的背景图一起淡化。
  const drawnBackground = skinPreviewBackground(
    background.drawn,
    preferences.candidate_opacity_percent,
  );
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
            preedit={preeditControlState(preferences).candidateVisible}
            helpcode={helpcode}
            brand={brand}
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
  brand,
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
  brand?: boolean;
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
  // 保存只触发后台重扫，不移除仍属于同一宿主和皮肤的预览；手动刷新继续重新读取图片。
  const source = useMemo(() => ({}), [scan, id, active, refresh]);
  const key = useMemo(() => ({}), [source, revision]);
  const [result, setResult] = useState<{ source: object; skin?: ExternalSkin; failed?: boolean }>();
  const generation = useAsyncGeneration(scan, key, active, id);
  useEffect(() => {
    const current = generation.current;
    if (active && scan)
      void (async () => {
        try {
          const catalog = await scan();
          if (generation.current === current)
            setResult({ source, skin: catalog.packages.find((skin) => skin.id === id) });
        } catch {
          if (generation.current === current) setResult({ source, failed: true });
        }
      })();
  }, [key, source, active, scan, id, generation]);
  if (!active) return null;
  if (!scan)
    return (
      <StatusMessage role="status">当前宿主不支持扫描外部皮肤，无法预览所选皮肤。</StatusMessage>
    );
  const current = result?.source === source ? result : undefined;
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
          brand={brand}
          theme={theme}
          reserve={reserve}
        />
      )}
    </>
  );
}
