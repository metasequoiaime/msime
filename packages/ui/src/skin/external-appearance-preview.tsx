import { useEffect, useId, useMemo, useState, type CSSProperties } from "react";
import type { Preferences } from "../index";
import {
  dimension,
  useExternalSkinPalette,
  type ExternalSkin,
  type SkinCatalog,
} from "./external-skins";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { candidateFontSize, candidateFontStyle } from "../candidate/candidate-font-size";
import { candidateTextStyle } from "../candidate/candidate-text-color";
import { candidateFamilyStyle } from "../candidate/candidate-font-family";
import * as settings from "../settings/settings-style";

function LoadedPreview({
  skin,
  preferences,
  readImage,
  helpcode,
  theme,
}: {
  skin: ExternalSkin;
  preferences: Preferences;
  readImage?: SkinImageReader;
  helpcode: boolean;
  theme: "dark" | "light";
}) {
  const scope = `appearance-external-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const paletteFailed = useExternalSkinPalette(scope, skin.candidate, theme);
  const top = dimension(skin.decorationTopDip, 500),
    width = dimension(skin.decorationWidthDip, 1000);
  const decorated = top > 0 && width > 0;
  const image = useSkinImage(readImage, skin.id, decorated ? skin.preview : null, 0);
  const [decodeFailed, setDecodeFailed] = useState(false);
  useEffect(() => setDecodeFailed(false), [image]);
  const base = ["fluent", "wechat", "graphite", "willow_green"].includes(skin.base)
    ? skin.base
    : "fluent";
  const geometry = {
    ...candidateFontStyle(preferences),
    ...candidateTextStyle(
      preferences.candidate_text_color,
      preferences.candidate_number_color,
      preferences.candidate_accent_color,
      preferences.candidate_selected_color,
      preferences.candidate_hover_color,
      preferences.candidate_surface_color,
      preferences.candidate_border_color,
    ),
    ...candidateFamilyStyle(preferences),
    "--msime-skin-min-width": `${dimension(skin.minWidthDip, 1000)}px`,
    "--msime-skin-decoration-top": `${decorated ? top : 0}px`,
    "--msime-skin-decoration-width": `${decorated ? width : 0}px`,
  } as CSSProperties;
  return (
    <div className={decorated ? "external-skin-decorated" : undefined}>
      <div
        data-skin-preview=""
        className={`${settings.skinCardPreview} appearance-candidate-preview skin-${base} ${scope}`}
        style={geometry}
        data-preview-theme={theme}
        data-font-size={candidateFontSize(preferences.candidate_font_size)}
        aria-hidden="true"
      >
        <div className={settings.skinPreviewStage} data-skin-stage="">
          <SkinCandidatePreview
            orientation={preferences.candidate_layout ?? "vertical"}
            count={preferences.candidate_page_size}
            preedit={preferences.candidate_preedit_style !== "empty"}
            helpcode={helpcode}
            decorated={decorated}
            image={decodeFailed ? undefined : image?.url}
            onImageError={() => setDecodeFailed(true)}
          />
        </div>
      </div>
      {paletteFailed && <p role="status">当前浏览器无法应用皮肤配色，保留基础预览。</p>}
      {(image?.failed || decodeFailed) && (
        <p role="status">皮肤图片加载失败，保留基础预览。可刷新预览重试。</p>
      )}
      {decorated && skin.preview && !readImage && <p role="status">当前宿主不支持皮肤图片预览。</p>}
    </div>
  );
}

export function ExternalAppearancePreview({
  preferences,
  scan,
  readImage,
  active,
  revision,
  helpcode,
  theme,
}: {
  preferences: Preferences;
  scan?: () => Promise<SkinCatalog>;
  readImage?: SkinImageReader;
  active: boolean;
  revision: number;
  helpcode: boolean;
  theme: "dark" | "light";
}) {
  const [refresh, setRefresh] = useState(0);
  const id = preferences.candidate_skin;
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
  const compatible =
    skin?.layouts.includes(preferences.candidate_layout ?? "vertical") &&
    skin.themes.includes(theme);
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
          helpcode={helpcode}
          theme={theme}
        />
      )}
    </>
  );
}
