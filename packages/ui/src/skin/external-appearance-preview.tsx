import { useEffect, useId, useMemo, useState, type CSSProperties } from "react";
import type { Preferences } from "../index";
import {
  decorationImage,
  dimension,
  drawnPackagePalette,
  selectedBarCss,
  skinGeometryStyle,
  usePreviewBackground,
  type ExternalSkin,
  type SkinCatalog,
} from "./external-skins";
import { installSkinPalette } from "./skin-palette";
import {
  candidatePaletteStyle,
  customCandidatePalette,
  themeEntry,
  type ResolvedTheme,
  type ResolveThemeRequest,
} from "../theme/global-theme";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { candidateFontSize, candidateFontStyle } from "../candidate/candidate-font-size";
import { candidateFamilyStyle } from "../candidate/candidate-font-family";
import * as settings from "../settings/settings-style";

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
}: {
  skin: ExternalSkin;
  preferences: Preferences;
  readImage?: SkinImageReader;
  resolve?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  helpcode: boolean;
  theme: "dark" | "light";
}) {
  const scope = `appearance-external-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const [paletteFailed, setPaletteFailed] = useState(false);
  const layout = preferences.candidate_layout ?? "vertical";
  // The host's `resolve()` is the authority; until it answers, and on hosts without it, the page's mirror layers the base, the package palette for the drawn mode (none when the package does not declare it) and the pickers the same way.
  const resolved = useResolvedTheme(resolve, {
    global_theme: "custom",
    custom_theme: preferences.custom_theme,
    dark: theme === "dark",
    layout,
  });
  const palette = resolved
    ? resolved.candidate
    : customCandidatePalette(
        skin.base,
        preferences.custom_theme?.candidate_colors,
        drawnPackagePalette(skin, theme),
      );
  const hideBar = palette?.show_selected_bar === false;
  useEffect(() => {
    if (!hideBar) {
      setPaletteFailed(false);
      return;
    }
    try {
      const remove = installSkinPalette(selectedBarCss(scope, { showSelectedBar: false }));
      setPaletteFailed(false);
      return remove;
    } catch {
      setPaletteFailed(true);
    }
  }, [hideBar, scope]);
  const decorated =
    dimension(skin.decorationTopDip, 500) > 0 && dimension(skin.decorationWidthDip, 1000) > 0;
  const decoration = decorated ? decorationImage(skin) : null;
  const image = useSkinImage(readImage, skin.id, decoration, 0);
  const [decodeFailed, setDecodeFailed] = useState(false);
  useEffect(() => setDecodeFailed(false), [image]);
  const background = usePreviewBackground(readImage, skin, 0);
  const geometry = {
    ...candidatePaletteStyle(palette),
    ...candidateFontStyle(preferences),
    ...candidateFamilyStyle(preferences),
    ...skinGeometryStyle(skin, theme),
  } as CSSProperties;
  return (
    <div className={decorated ? "external-skin-decorated" : undefined}>
      <div
        data-skin-preview=""
        className={`${settings.skinCardPreview} appearance-candidate-preview ${scope}`}
        style={geometry}
        data-preview-theme={theme}
        data-decoration-align={skin.decorationAlign ?? "right"}
        data-font-size={candidateFontSize(preferences.candidate_font_size)}
        aria-hidden="true"
      >
        <div className={settings.skinPreviewStage} data-skin-stage="">
          <SkinCandidatePreview
            orientation={layout}
            count={preferences.candidate_page_size}
            preedit={preferences.candidate_preedit_style !== "empty"}
            helpcode={helpcode}
            decorated={decorated}
            image={decodeFailed ? undefined : image?.url}
            onImageError={() => setDecodeFailed(true)}
            background={background.drawn}
            onBackgroundError={background.onError}
          />
        </div>
      </div>
      {paletteFailed && <p role="status">当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。</p>}
      {(image?.failed || decodeFailed || background.failed) && (
        <p role="status">皮肤图片加载失败，保留基础预览。可刷新预览重试。</p>
      )}
      {(decoration || skin.background) && !readImage && (
        <p role="status">当前宿主不支持皮肤图片预览。</p>
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
}: {
  preferences: Preferences;
  scan?: () => Promise<SkinCatalog>;
  readImage?: SkinImageReader;
  /** `SettingsClient.resolveTheme`, when the host has it. */
  resolve?: (request: ResolveThemeRequest) => Promise<ResolvedTheme>;
  active: boolean;
  revision: number;
  helpcode: boolean;
  theme: "dark" | "light";
}) {
  const [refresh, setRefresh] = useState(0);
  const id = preferences.custom_theme?.candidate_skin;
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
  if (!scan) return <p role="status">当前宿主不支持扫描外部皮肤，无法预览所选皮肤。</p>;
  const current = result?.key === key ? result : undefined;
  const skin = current?.skin;
  // A built-in base fixes the mode whatever the host draws in, as `resolve()` does: the package palette for that mode is used when the package declares it, and the base alone is drawn when it does not.
  const fixed = skin ? themeEntry(skin.base).appearance : null;
  const drawn = fixed ?? theme;
  const compatible =
    skin?.layouts.includes(preferences.candidate_layout ?? "vertical") &&
    (fixed !== null || skin.themes.includes(theme));
  return (
    <>
      <button
        type="button"
        className="skin-preview-switch"
        disabled={!current}
        onClick={() => setRefresh((value) => value + 1)}
      >
        刷新预览
      </button>
      {!current ? (
        <p role="status">正在读取所选皮肤。</p>
      ) : current.failed ? (
        <p role="status">读取所选皮肤失败，请刷新预览重试。</p>
      ) : !skin ? (
        <p role="status">未找到所选皮肤，请检查皮肤目录后刷新预览。</p>
      ) : !compatible ? (
        <p role="status">所选皮肤不支持当前布局或{theme === "dark" ? "深色" : "浅色"}模式。</p>
      ) : (
        <LoadedPreview
          skin={skin}
          preferences={preferences}
          readImage={readImage}
          resolve={resolve}
          helpcode={helpcode}
          theme={drawn}
        />
      )}
    </>
  );
}
