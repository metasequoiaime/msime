import { useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { subscribeToWindowResume } from "../core/window-focus";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { SkinToolbarPreview } from "./skin-toolbar-preview";
import { SkinImageToolbarPreview } from "./skin-image-toolbar-preview";
import type { SkinToolbarImage } from "./toolbar-image-layout";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import type { SkinFontReader } from "./skin-font";
import { useSelectedBarPalette } from "./skin-palette";
export { selectedBarCss } from "./skin-palette";
import { useToolbarCss, type ToolbarCssReader } from "./use-toolbar-css";
import { SkinCardHeader } from "./skin-card-header";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";
import { dimension, skinDecorationGeometry, skinPreviewBackground } from "./skin-preview-geometry";
export { dimension } from "./skin-preview-geometry";
import { useDecodedSkinImage } from "./skin-image";
import { skinCanvasGeometry, canvasVariables } from "./skin-canvas-preview";
import { candidateWindowStyle } from "../candidate/candidate-window-style";
import type { CandidateWindowStylePreferences } from "../candidate/candidate-preferences";
import * as settings from "../settings/settings-style";
import { SettingsExternalMeta } from "../settings/settings-external-meta";
import { SettingsGroupBlock } from "../settings/settings-group-block";
import { Row } from "../core/platform-controls";
import { clamp } from "../core/number";
import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import { subscribeSkinCatalogChanges } from "./skin-catalog-changes";
import { useAsyncGeneration } from "../settings/use-async-generation";
import {
  customCandidateStyle,
  normalizedColor,
  skinSlot,
  themeEntry,
  type BaseGlobalTheme,
  type PackageCandidatePalette,
} from "../theme/global-theme";

type Palette = PackageCandidatePalette;
export type SkinBackground = {
  /** Package-relative; read through the host image reader. */
  image: string;
  fit: "cover" | "contain" | "stretch";
  opacity: number;
};
/** One mode of the manifest's `[toolbar]` colours, normalized by the scan. */
export type ToolbarPalette = Partial<
  Record<"background" | "border" | "handle" | "divider" | "icon" | "hover", string | null>
>;
/** 清单 `candidate_window` 的四边长度，单位 DIP。 */
export type SkinInsets = { top: number; right: number; bottom: number; left: number };
/** 清单 `[candidate_window.border_image]` 九宫格面板；`sliceDip` 以源图像素计，写了 `stretchDip` 就用它当切片，`widthDip` 缺省取切片。 */
export type SkinBorderImage = {
  source: string;
  lightSource?: string | null;
  sliceDip: number[];
  stretchDip?: number[] | null;
  widthDip?: number[] | null;
  fill?: boolean | null;
};
/** 清单 `[[candidate_window.decorations]]` 自由挂件；框按 `anchor` 贴卡片，再平移 `offsetXDip` / `offsetYDip`，图片在框内等比缩放。 */
export type SkinFreeDecoration = {
  image: string;
  anchor?: string | null;
  offsetXDip?: number | null;
  offsetYDip?: number | null;
  widthDip: number;
  heightDip: number;
  zIndex?: number | null;
};
export type ExternalSkin = {
  id: string;
  /** 清单版本；版本 2 对任何 base 都增加有限的候选窗动效契约（`animation`、装饰 `motion` 与 APNG 帧）。 */
  schemaVersion?: 1 | 2;
  /** 清单写的是 `base = "canvas"`（此时 `base` 报成 `system`）。九宫格、内容与外留白、拼音行距和自由挂件由设置页的卡片预览画出；不影响动效。 */
  canvas?: boolean;
  /** 不影响加载的提示，对应 msime-windows 的 `package.notices`，例如非 canvas 基底上被忽略的 canvas 专属键。 */
  notices?: string[];
  name: string;
  version: string;
  /** The global theme under the package's own colours; `system` for none. The manifest parser never gives `custom`. */
  base: BaseGlobalTheme;
  author: string | null;
  description: string | null;
  layouts: string[];
  themes: string[];
  minWidthDip: number;
  /** The card radius, 0-32; `null` keeps the preview's own. The keys below are optional because a document from before they existed has none of them. */
  cornerRadiusDip?: number | null;
  decorationTopDip: number;
  decorationWidthDip: number;
  /** The image drawn in the decoration band, set only for a decorated package. */
  decorationImage?: string | null;
  decorationAlign?: "left" | "center" | "right";
  /** 候选窗在屏幕上时装饰图的待机动效，由原生候选窗播放；设置页的预览不播放。 */
  decorationMotion?: "none" | "float" | "sway" | "breathe" | "bounce";
  decorationMotionPeriodMs?: number;
  /** 装饰图的帧数：没有装饰图为 0，静态图为 1，APNG（以及手动装进皮肤目录的 GIF、WebP）为它的动画帧数。 */
  decorationFrames?: number;
  /** 装饰图内的候选词叠加声明，v1、v2 均可用。 */
  decorationCandidate?: import("./decoration-candidate").DecorationCandidate | null;
  background?: SkinBackground | null;
  /** `[candidate_window.border_image]` 九宫格面板；只在 `canvas` 时由 catalog 保留。 */
  borderImage?: SkinBorderImage | null;
  /** `candidate_window.content_insets_dip`：面板外缘到内容的内边距。 */
  contentInsetsDip?: SkinInsets | null;
  /** `candidate_window.line_gap_dip`：拼音行与候选之间的距离。 */
  lineGapDip?: number | null;
  /** `candidate_window.outer_insets_dip`：卡片四周的窗口留白。 */
  outerInsetsDip?: SkinInsets | null;
  /** `[[candidate_window.decorations]]` 自由挂件，最多 32 个。 */
  decorations?: SkinFreeDecoration[];
  /** 面板成功绘制时使用的贴图配色；`candidate` 仍是未绘制贴图时的可读回退。 */
  panelCandidate?: { dark: Palette; light: Palette };
  toolbar?: { cornerRadiusDip: number | null; dark: ToolbarPalette; light: ToolbarPalette };
  toolbarImage?: SkinToolbarImage;
  toolbarStylesheet: string | null;
  preview: string | null;
  candidate: { dark: Palette; light: Palette };
  animation?: {
    enter: "none" | "fade" | "fade_scale" | "slide_up" | "spring" | "glow";
    exit: "none" | "fade" | "fade_scale" | "slide_up" | "spring" | "glow";
    selection: "none" | "fade" | "fade_scale" | "slide_up" | "spring" | "glow";
    durationMs: number;
    staggerMs: number;
  };
  license?: { code: string | null; assets: string | null; source: string | null } | null;
};
export type SkinCatalog = {
  directory: string;
  packages: ExternalSkin[];
  issues: { folder: string; reason: string }[];
};

/** The decoration band's image as the scan resolved it; a document without `decorationImage` predates it and drew its preview there. */
export function decorationImage(
  skin: Pick<ExternalSkin, "decorationImage" | "preview">,
): string | null {
  return skin.decorationImage === undefined ? skin.preview : skin.decorationImage;
}

/** 指定主题下的包预览几何和工具栏属性；清单颜色写入样式前再次校验。 */
export function skinGeometryStyle(
  skin: ExternalSkin,
  theme: "dark" | "light",
): Record<string, string> {
  const { top, width } = skinDecorationGeometry(skin);
  const style: Record<string, string> = {
    "--msime-skin-min-width": `${dimension(skin.minWidthDip, 1000)}px`,
    "--msime-skin-decoration-top": `${top}px`,
    "--msime-skin-decoration-width": `${width}px`,
  };
  if (typeof skin.cornerRadiusDip === "number")
    style["--msime-skin-radius"] = `${dimension(skin.cornerRadiusDip, 32)}px`;
  const toolbar = skin.toolbar;
  if (typeof toolbar?.cornerRadiusDip === "number")
    style["--msime-toolbar-radius"] = `${dimension(toolbar.cornerRadiusDip, 32)}px`;
  const colors = toolbar?.[theme] ?? {};
  for (const key of ["background", "border", "handle", "divider", "icon", "hover"] as const) {
    const value = colors[key];
    const color = typeof value === "string" ? normalizedColor(value) : null;
    if (color) style[`--msime-toolbar-${key}`] = color;
  }
  return style;
}

/** The package palette `resolve()` draws for `theme`: the declared one, or none, in which case the base is drawn alone. A package is never layered over its other mode. */
export function drawnPackagePalette(
  skin: Pick<ExternalSkin, "themes" | "candidate">,
  theme: "dark" | "light",
): Palette | null {
  return skin.themes.includes(theme) ? skin.candidate[theme] : null;
}

/** 皮肤放在哪个槽位、用在哪种明暗下，写给用户看：浅色皮肤只用在浅色模式，深色皮肤只用在深色模式，`system` 底的两种都能用（`skinSlot`）。 */
export function skinSlotLabel(base: BaseGlobalTheme): string {
  const slot = skinSlot(base);
  return slot === "light"
    ? "浅色皮肤，用在浅色模式"
    : slot === "dark"
      ? "深色皮肤，用在深色模式"
      : "跟随明暗，浅色、深色模式都能用";
}

/** The package background read through the host image reader, ready for `SkinCandidatePreview`; nothing is drawn while it loads, when it fails to load or decode, or without a reader. */
export function usePreviewBackground(
  readImage: SkinImageReader | undefined,
  skin: Pick<ExternalSkin, "id" | "background">,
  revision: number,
) {
  const background = skin.background ?? null;
  const image = useSkinImage(readImage, skin.id, background?.image ?? null, revision);
  const [failedImage, setFailedImage] = useState<ReturnType<typeof useSkinImage>>();
  const decodeFailed = image !== undefined && failedImage === image;
  const drawn =
    background && image?.url && !decodeFailed
      ? {
          url: image.url,
          fit: background.fit,
          opacity: clamp(Number(background.opacity) || 0, 0, 1),
        }
      : undefined;
  return {
    drawn,
    failed: Boolean(image?.failed || decodeFailed),
    onError: () => setFailedImage(image),
  };
}

/** Shared image and background loading state for external skin previews. */
export function useSkinPreviewAssets(
  readImage: SkinImageReader | undefined,
  skin: Pick<ExternalSkin, "id" | "background">,
  decoration: string | null,
  revision: number,
) {
  const image = useSkinImage(readImage, skin.id, decoration, revision);
  const [failedImage, setFailedImage] = useState<ReturnType<typeof useSkinImage>>();
  const decodeFailed = image !== undefined && failedImage === image;
  const background = usePreviewBackground(readImage, skin, revision);
  return {
    image,
    decodeFailed,
    onImageError: () => setFailedImage(image),
    background,
  };
}

/** One scanned package as a card of the theme carousel, drawn with the built-in theme cards' own styles. */
export function ExternalSkinCard({
  skin,
  selected,
  layout,
  onSelect,
  onDeselect,
  readImage,
  readFont,
  readToolbarCss,
  revision,
  activeTheme,
  toolbarPreview,
  onPublish,
  windowStyle,
}: {
  skin: ExternalSkin;
  /** 自定义主题正在使用，且这个包在浅色或深色槽位里。 */
  selected: boolean;
  layout: string;
  /** Choosing a package passes its manifest base, which the custom theme is then drawn over. */
  onSelect: (id: string, base: ExternalSkin["base"]) => void;
  /** 关掉已在使用的包，把它从所在的槽位取下；不提供时开关在选中状态下再点一次仍是 `onSelect`。 */
  onDeselect?: (id: string) => void;
  readImage?: SkinImageReader;
  readFont?: SkinFontReader;
  readToolbarCss?: ToolbarCssReader;
  revision: number;
  activeTheme: "dark" | "light";
  /** The host draws a floating toolbar the skin styles. The Linux hosts present the toolbar as an input method menu, so their cards preview only the candidate window. */
  toolbarPreview: boolean;
  /** Offers the package to the community; absent on hosts without the candidate-skin community. */
  onPublish?: (id: string) => void;
  /** 用户在「候选窗口」页设的窗口圆角和不透明度，卡片和候选窗一样画在包的几何之上；不传时按包自己的画。 */
  windowStyle?: CandidateWindowStylePreferences;
}) {
  const [override, setOverride] = useState<"dark" | "light" | null>(null);
  useEffect(() => setOverride(null), [activeTheme]);
  // A built-in base fixes the mode whatever the host draws in, as `resolve()` does: the package palette for that mode is used when the package declares it, and the base alone is drawn when it does not.
  const fixed = themeEntry(skin.base).appearance;
  const theme =
    fixed ??
    override ??
    (skin.themes.includes(activeTheme)
      ? activeTheme
      : skin.themes[0] === "light"
        ? "light"
        : "dark");
  const scope = `external-preview-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const toolbarState = useToolbarCss(
    toolbarPreview ? readToolbarCss : undefined,
    skin.id,
    skin.toolbarStylesheet,
    revision,
    scope,
    readImage,
    readFont,
  );
  const canvas = skinCanvasGeometry(skin, theme);
  const panel = useDecodedSkinImage(readImage, skin.id, canvas?.panel?.image ?? null, revision);
  const palette =
    panel?.url && skin.themes.includes(theme)
      ? (skin.panelCandidate?.[theme] ?? drawnPackagePalette(skin, theme))
      : drawnPackagePalette(skin, theme);
  const hideBar = palette?.showSelectedBar === false;
  const paletteFailed = useSelectedBarPalette(scope, hideBar);
  // 卡片上的预览切换不影响能否选用。固定明暗底的包放进自己的槽位、只在那种明暗下画，所以当前模式挡不住它；`system` 底的包两个槽位都放，仍要支持当前模式。
  const compatible =
    skin.layouts.includes(layout) && (fixed !== null || skin.themes.includes(activeTheme));
  // 已在使用的包总能关掉，哪怕布局变了以后它不再能选。
  const deselecting = selected && onDeselect !== undefined;
  const { decorated } = skinDecorationGeometry(skin);
  const decoration = decorated ? decorationImage(skin) : null;
  // 所有 canvas 包都要 `.containerParent` 承载外留白、装饰带和挂件层；只画装饰带的包也照旧要它。
  const framed = decorated || canvas !== null;
  const { image, decodeFailed, onImageError, background } = useSkinPreviewAssets(
    readImage,
    skin,
    decoration,
    revision,
  );
  const geometry = {
    // The package drawn over its base theme, as `resolve()` layers them; a card has no pickers.
    ...customCandidateStyle(skin.base, undefined, palette),
    ...skinGeometryStyle(skin, theme),
    // canvas 的内容／外留白和行距写在预览表面上，候选标记里的选择器按继承读到它们。
    ...(canvas ? canvasVariables(canvas) : {}),
    // 放在包的几何之后：用户设的圆角压过包的圆角，与各宿主相同。
    ...candidateWindowStyle(windowStyle ?? {}),
  } as CSSProperties;
  // 窗口不透明度连同包的背景图一起淡化，背景图本来就画在卡片底色上。
  const drawnBackground = skinPreviewBackground(
    background.drawn,
    windowStyle?.candidate_opacity_percent,
  );
  const note = `${settings.skinCardDescription} ${settings.externalResourceNote}`;
  return (
    <article
      aria-label={skin.name}
      // `external-skin-decorated` is the `@utility` that lays out the decoration band inside the preview; canvas cards use it for the outer margins and decoration layers too.
      className={`${settings.skinCard(selected)}${framed ? " external-skin-decorated" : ""}`}
    >
      <SkinCardHeader
        title={skin.name}
        theme={theme}
        selected={selected}
        description={
          compatible
            ? skin.description || `基于 ${themeEntry(skin.base).title}`
            : `当前布局或明暗模式不受支持（${skin.layouts.join("/")}，${skin.themes.join("/")}）`
        }
        details={
          <>
            <SettingsExternalMeta as="span">{skinSlotLabel(skin.base)}</SettingsExternalMeta>
            <SettingsExternalMeta as="span">
              {[skin.id, skin.version && `v${skin.version}`, skin.author]
                .filter(Boolean)
                .join(" · ")}
            </SettingsExternalMeta>
          </>
        }
        actions={
          <>
            <ActionButton
              action={() => (deselecting ? onDeselect(skin.id) : onSelect(skin.id, skin.base))}
              ariaChecked={selected}
              ariaLabel={skin.name}
              className={settings.skinSwitch(selected)}
              disabled={!compatible && !deselecting}
              label={<span className={settings.skinSwitchKnob(selected)} />}
              role="switch"
            />
            {fixed === null && (
              <ActionButton
                action={() => setOverride(theme === "dark" ? "light" : "dark")}
                className={settings.skinPreviewSwitch}
                label={theme === "dark" ? "预览浅色" : "预览深色"}
              />
            )}
            {onPublish && (
              <ActionButton
                action={() => onPublish(skin.id)}
                className={settings.skinPreviewSwitch}
                label="发布到社区"
              />
            )}
          </>
        }
      />
      <SkinPreviewSurface
        className={`${scope}${theme === "light" ? " theme-light" : ""}`}
        style={geometry}
        data-preview-theme={theme}
        data-decoration-align={skin.decorationAlign ?? "right"}
        aria-hidden="true"
      >
        <div className={settings.skinCandidateStages}>
          <SkinPreviewStage>
            <SkinCandidatePreview
              orientation="horizontal"
              decorated={decorated}
              canvas={canvas}
              panelUrl={panel?.url}
              readImage={readImage}
              skinId={skin.id}
              revision={revision}
              image={decodeFailed ? undefined : image?.url}
              onImageError={onImageError}
              decorationCandidate={skin.decorationCandidate}
              background={drawnBackground}
              onBackgroundError={background.onError}
            />
          </SkinPreviewStage>
          <SkinPreviewStage>
            <SkinCandidatePreview
              orientation="vertical"
              decorated={decorated}
              canvas={canvas}
              panelUrl={panel?.url}
              readImage={readImage}
              skinId={skin.id}
              revision={revision}
              image={decodeFailed ? undefined : image?.url}
              onImageError={onImageError}
              decorationCandidate={skin.decorationCandidate}
              background={drawnBackground}
              onBackgroundError={background.onError}
            />
          </SkinPreviewStage>
        </div>
        {toolbarPreview && (
          <SkinPreviewStage>
            {skin.toolbarImage ? (
              <SkinImageToolbarPreview
                skin={skin.toolbarImage}
                id={skin.id}
                readImage={readImage}
                revision={revision}
                theme={theme}
              />
            ) : (
              <SkinToolbarPreview />
            )}
          </SkinPreviewStage>
        )}
      </SkinPreviewSurface>
      {paletteFailed && (
        <StatusMessage role="status" className={note}>
          当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。
        </StatusMessage>
      )}
      {(image?.failed || decodeFailed || background.failed) && (
        <StatusMessage role="status" className={note}>
          皮肤图片加载失败，保留基础预览。可刷新皮肤重试。
        </StatusMessage>
      )}
      {(decoration || skin.background) && !readImage && (
        <p className={note}>当前宿主不支持皮肤图片预览。</p>
      )}
      {toolbarPreview && skin.toolbarStylesheet && !readToolbarCss && (
        <p className={note}>当前宿主不支持外部工具栏样式。</p>
      )}
      {toolbarState === "failed" && (
        <StatusMessage role="status" className={note}>
          工具栏样式加载失败，保留基础预览。可刷新皮肤重试。
        </StatusMessage>
      )}
      {toolbarState === "partial" && (
        <StatusMessage role="status" className={note}>
          已应用工具栏基础样式；关联资源及部分规则尚未支持。
        </StatusMessage>
      )}
    </article>
  );
}

export type SkinCatalogState = {
  /** The last catalog a scan returned; a failed scan keeps it. */
  catalog: SkinCatalog | null;
  /** Bumped by every scan that lands, so package images are read again even when the manifests did not change. */
  revision: number;
  busy: boolean;
  failed: boolean;
  refresh: () => Promise<void>;
  opening: boolean;
  openFailed: boolean;
  /** Opens the skin directory, or imports a skin on a host that copies one in. */
  openFolder: () => Promise<void>;
};

/**
 * The scanned external skin catalog and the directory actions around it, for the theme page's carousel and the 外部皮肤 row.
 *
 * `importsSkin`: the host copies a skin the user points at, instead of opening a folder for them to drop one into. Its skin folder is inside an application sandbox, so there is nothing to open, and the catalog is scanned again once the import answers.
 *
 * `active`：列出目录的页面当前是否显示。设置页切走后仍挂在隐藏的 fieldset 里，所以页面重新显示、窗口重新获得焦点（用户可能刚在 Finder 里拷进皮肤，或 MCP 服务刚生成了一款）时各重扫一次；社区图库装入或同步改动了目录（`notifySkinCatalogChanged`）时，无论页面是否显示都重扫，切回来时列表已经是新的。
 */
export function useSkinCatalog(
  scan: (() => Promise<SkinCatalog>) | undefined,
  openDirectory: (() => Promise<void>) | undefined,
  importsSkin: boolean,
  active = true,
): SkinCatalogState {
  const [catalog, setCatalog] = useState<SkinCatalog | null>(null);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const generation = useAsyncGeneration(scan);
  const pending = useRef(false);
  // 扫描进行中又收到目录改动的通知：进行中的那次可能读不到新皮肤，结束后再补扫一次。
  const queued = useRef(false);
  // 最近一次落地的清单，自动重扫据此判断结果有没有变化。
  const latest = useRef<SkinCatalog | null>(null);
  const [opening, setOpening] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const openGeneration = useAsyncGeneration(openDirectory);
  const openPending = useRef(false);
  useEffect(() => {
    openPending.current = false;
    setOpening(false);
    setOpenFailed(false);
    return () => {
      openPending.current = false;
    };
  }, [openDirectory]);
  async function openFolder() {
    if (!openDirectory || openPending.current) return;
    openPending.current = true;
    const current = ++openGeneration.current;
    setOpening(true);
    setOpenFailed(false);
    try {
      await openDirectory();
      // List what the import brought in rather than make the user refresh for something they just did. iOS answers once the copy is in place; HarmonyOS answers as the picker opens, so there the list catches up on the user's own refresh. A host that opens a folder has only started the user's own copy.
      if (importsSkin && current === openGeneration.current) void refresh();
    } catch {
      if (current === openGeneration.current) setOpenFailed(true);
    } finally {
      if (current === openGeneration.current) {
        openPending.current = false;
        setOpening(false);
      }
    }
  }
  useEffect(() => {
    pending.current = false;
    queued.current = false;
    latest.current = null;
    setCatalog(null);
    setBusy(false);
    setFailed(false);
    // Scan as the page opens, as the native fallback page (SkinSettingsView) does. Waiting for a manual refresh left the carousel without the package in use, so the skin in use looked missing.
    void run(true);
    return () => {
      pending.current = false;
      queued.current = false;
    };
  }, [scan]);
  /**
   * 扫描一次目录，任何时候最多只有一次在进行。`manual` 的结果总会落地并递增 `revision`，让清单没变时也重读皮肤图片；自动重扫的结果和上次相同时保留原清单，卡片不重绘，也不重读图片。
   */
  async function run(manual: boolean) {
    if (!scan || pending.current) return;
    pending.current = true;
    queued.current = false;
    const current = ++generation.current;
    setBusy(true);
    setFailed(false);
    try {
      const result = await scan();
      if (
        current === generation.current &&
        (manual || JSON.stringify(result) !== JSON.stringify(latest.current))
      ) {
        latest.current = result;
        setCatalog(result);
        setRevision((value) => value + 1);
      }
    } catch {
      if (current === generation.current) setFailed(true);
    } finally {
      if (current === generation.current) {
        pending.current = false;
        if (queued.current) void run(false);
        else setBusy(false);
      }
    }
  }
  function refresh() {
    return run(true);
  }
  // 自动重扫走最新一次渲染的 `run`，订阅本身不必随每次渲染重建。
  const rescan = useLatestRef(run);
  useEffect(
    () =>
      subscribeSkinCatalogChanges(() => {
        if (pending.current) queued.current = true;
        else void rescan.current(false);
      }),
    [],
  );
  // 页面重新显示时重扫。挂载时的那次已由上面的扫描负责，这里只看从隐藏到显示的变化。
  const shown = useRef(active);
  useEffect(() => {
    const wasShown = shown.current;
    shown.current = active;
    if (active && !wasShown) void rescan.current(false);
  }, [active]);
  // 页面显示期间，窗口重新获得焦点或从后台回到前台时重扫。两个事件常常一起到达，第二个碰上进行中的扫描就直接放弃。
  useEffect(() => {
    if (!active) return;
    return subscribeToWindowResume(() => void rescan.current(false));
  }, [active]);
  return { catalog, revision, busy, failed, refresh, opening, openFailed, openFolder };
}

/** The 外部皮肤 row and its diagnostics: where external skins come from (a folder to open, or an import on a host that copies one in) and what the last scan found. It goes inside a `GroupList`. */
export function ExternalSkinDirectoryRow({
  skins,
  scannable,
  openable,
  importsSkin,
  status,
}: {
  skins: SkinCatalogState;
  /** The host has a skin scanner. */
  scannable: boolean;
  /** The host can open the skin directory, or import a skin. */
  openable: boolean;
  importsSkin: boolean;
  /** More about the directory under the scan result, such as how it last synced with the user's library. */
  status?: ReactNode;
}) {
  return (
    <>
      <Row
        title="外部皮肤"
        description={
          importsSkin
            ? "点“导入皮肤”，选中包含 skin.toml 的皮肤文件夹。文件夹名只能用小写字母、数字和 . _ -，同名皮肤会被替换。"
            : "把包含 skin.toml 的皮肤文件夹复制到下面的目录，回到这个窗口时会自动列出。"
        }
        detail={
          <>
            <span role="status" className="block">
              {!scannable
                ? "当前宿主不支持扫描外部皮肤。"
                : skins.busy
                  ? "正在读取皮肤目录。"
                  : !skins.catalog
                    ? "尚未扫描。点击“刷新皮肤”读取皮肤目录。"
                    : !skins.catalog.packages.length
                      ? "没有发现外部皮肤。"
                      : ""}
            </span>
            {status}
            {/* 目录只在要手动把皮肤文件夹复制进去时有用；能用「导入皮肤」的宿主（手机）上它是应用沙箱里的内部路径，用户既看不到也用不上，不显示。 */}
            {!importsSkin && skins.catalog?.directory && (
              <code className={settings.externalDirectory} title={skins.catalog.directory}>
                {skins.catalog.directory}
              </code>
            )}
          </>
        }
      >
        <ActionButton
          action={() => void skins.openFolder()}
          className="secondary"
          disabled={!openable || skins.opening}
          label={
            skins.opening
              ? importsSkin
                ? "正在导入…"
                : "正在打开…"
              : importsSkin
                ? "导入皮肤"
                : "打开目录"
          }
        />
        <ActionButton
          action={() => void skins.refresh()}
          className="secondary"
          disabled={!scannable || skins.busy}
          label={skins.busy ? "正在扫描…" : "刷新皮肤"}
        />
      </Row>
      {(skins.openFailed || skins.failed || !!skins.catalog?.issues.length) && (
        <SettingsGroupBlock>
          {skins.openFailed && (
            <SettingsExternalMeta role="alert">
              {importsSkin ? "导入皮肤失败，请重试。" : "无法打开皮肤目录，请重试。"}
            </SettingsExternalMeta>
          )}
          {skins.failed && (
            <SettingsExternalMeta role="alert">
              读取皮肤目录失败，请重试。{skins.catalog && "仍显示上次扫描结果。"}
            </SettingsExternalMeta>
          )}
          {!!skins.catalog?.issues.length && (
            <details className={settings.externalDiagnostics}>
              <summary>已忽略 {skins.catalog.issues.length} 个无效皮肤目录</summary>
              <ul>
                {skins.catalog.issues.map((issue, index) => (
                  <li key={index}>
                    {issue.folder}：{issue.reason}
                  </li>
                ))}
              </ul>
            </details>
          )}
        </SettingsGroupBlock>
      )}
    </>
  );
}
