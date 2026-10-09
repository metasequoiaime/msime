import { useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { SkinToolbarPreview } from "./skin-toolbar-preview";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import type { SkinFontReader } from "./skin-font";
import { useSelectedBarPalette } from "./skin-palette";
export { selectedBarCss } from "./skin-palette";
import { useToolbarCss, type ToolbarCssReader } from "./use-toolbar-css";
import { SkinCardHeader } from "./skin-card-header";
import { SkinPreviewStage } from "./skin-preview-stage";
import { SkinPreviewSurface } from "./skin-preview-surface";
import * as settings from "../settings/settings-style";
import { SettingsExternalMeta } from "../settings/settings-external-meta";
import { SettingsGroupBlock } from "../settings/settings-group-block";
import { Row } from "../core/platform-controls";
import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import { subscribeSkinCatalogChanges } from "./skin-catalog-changes";
import { useAsyncGeneration } from "../settings/use-async-generation";
import {
  customCandidateStyle,
  normalizedColor,
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
export type ExternalSkin = {
  id: string;
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
  background?: SkinBackground | null;
  toolbar?: { cornerRadiusDip: number | null; dark: ToolbarPalette; light: ToolbarPalette };
  toolbarStylesheet: string | null;
  preview: string | null;
  candidate: { dark: Palette; light: Palette };
  license?: { code: string | null; assets: string | null; source: string | null } | null;
};
export type SkinCatalog = {
  directory: string;
  packages: ExternalSkin[];
  issues: { folder: string; reason: string }[];
};

export function dimension(value: number, maximum: number): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= maximum
    ? value
    : 0;
}

/** The decoration band's image as the scan resolved it; a document without `decorationImage` predates it and drew its preview there. */
export function decorationImage(
  skin: Pick<ExternalSkin, "decorationImage" | "preview">,
): string | null {
  return skin.decorationImage === undefined ? skin.preview : skin.decorationImage;
}

/** The preview's geometry and toolbar properties for a package drawn in `theme`: minimum width, decoration band, card radius and the `[toolbar]` radius and colours. Colours are checked again before they reach a style, as every value from a manifest is. */
export function skinGeometryStyle(
  skin: ExternalSkin,
  theme: "dark" | "light",
): Record<string, string> {
  const top = dimension(skin.decorationTopDip, 500);
  const width = dimension(skin.decorationWidthDip, 1000);
  const decorated = top > 0 && width > 0;
  const style: Record<string, string> = {
    "--msime-skin-min-width": `${dimension(skin.minWidthDip, 1000)}px`,
    "--msime-skin-decoration-top": `${decorated ? top : 0}px`,
    "--msime-skin-decoration-width": `${decorated ? width : 0}px`,
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

/** The package background read through the host image reader, ready for `SkinCandidatePreview`; nothing is drawn while it loads, when it fails to load or decode, or without a reader. */
export function usePreviewBackground(
  readImage: SkinImageReader | undefined,
  skin: Pick<ExternalSkin, "id" | "background">,
  revision: number,
) {
  const background = skin.background ?? null;
  const image = useSkinImage(readImage, skin.id, background?.image ?? null, revision);
  const [decodeFailed, setDecodeFailed] = useState(false);
  useEffect(() => setDecodeFailed(false), [image]);
  const drawn =
    background && image?.url && !decodeFailed
      ? {
          url: image.url,
          fit: background.fit,
          opacity: Math.min(1, Math.max(0, Number(background.opacity) || 0)),
        }
      : undefined;
  return {
    drawn,
    failed: Boolean(image?.failed || decodeFailed),
    onError: () => setDecodeFailed(true),
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
  const [decodeFailed, setDecodeFailed] = useState(false);
  useEffect(() => setDecodeFailed(false), [image]);
  const background = usePreviewBackground(readImage, skin, revision);
  return {
    image,
    decodeFailed,
    onImageError: () => setDecodeFailed(true),
    background,
  };
}

/** One scanned package as a card of the theme carousel, drawn with the built-in theme cards' own styles. */
export function ExternalSkinCard({
  skin,
  selected,
  layout,
  onSelect,
  readImage,
  readFont,
  readToolbarCss,
  revision,
  activeTheme,
  toolbarPreview,
  onPublish,
}: {
  skin: ExternalSkin;
  /** The custom theme is in use and draws this package. */
  selected: boolean;
  layout: string;
  /** Choosing a package passes its manifest base, which the custom theme is then drawn over. */
  onSelect: (id: string, base: ExternalSkin["base"]) => void;
  readImage?: SkinImageReader;
  readFont?: SkinFontReader;
  readToolbarCss?: ToolbarCssReader;
  revision: number;
  activeTheme: "dark" | "light";
  /** The host draws a floating toolbar the skin styles. The Linux hosts present the toolbar as an input method menu, so their cards preview only the candidate window. */
  toolbarPreview: boolean;
  /** Offers the package to the community; absent on hosts without the candidate-skin community. */
  onPublish?: (id: string) => void;
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
  const palette = drawnPackagePalette(skin, theme);
  const hideBar = palette?.showSelectedBar === false;
  const paletteFailed = useSelectedBarPalette(scope, hideBar);
  // Card-only overrides must not change runtime compatibility or selection. A package over a built-in base is drawn in that base's mode, so the host mode does not rule it out.
  const compatible =
    skin.layouts.includes(layout) && (fixed !== null || skin.themes.includes(activeTheme));
  const decorated =
    dimension(skin.decorationTopDip, 500) > 0 && dimension(skin.decorationWidthDip, 1000) > 0;
  const decoration = decorated ? decorationImage(skin) : null;
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
  } as CSSProperties;
  const note = `${settings.skinCardDescription} ${settings.externalResourceNote}`;
  return (
    <article
      aria-label={skin.name}
      // `external-skin-decorated` is the `@utility` that lays out the decoration band inside the preview.
      className={`${settings.skinCard(selected)}${decorated ? " external-skin-decorated" : ""}`}
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
          <SettingsExternalMeta as="span">
            {[skin.id, skin.version && `v${skin.version}`, skin.author].filter(Boolean).join(" · ")}
          </SettingsExternalMeta>
        }
        actions={
          <>
            <ActionButton
              action={() => onSelect(skin.id, skin.base)}
              ariaChecked={selected}
              ariaLabel={skin.name}
              className={settings.skinSwitch(selected)}
              disabled={!compatible}
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
              image={decodeFailed ? undefined : image?.url}
              onImageError={onImageError}
              background={background.drawn}
              onBackgroundError={background.onError}
            />
          </SkinPreviewStage>
          <SkinPreviewStage>
            <SkinCandidatePreview
              orientation="vertical"
              decorated={decorated}
              image={decodeFailed ? undefined : image?.url}
              onImageError={onImageError}
              background={background.drawn}
              onBackgroundError={background.onError}
            />
          </SkinPreviewStage>
        </div>
        {toolbarPreview && (
          <SkinPreviewStage>
            <SkinToolbarPreview />
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
  const rescan = useRef(run);
  rescan.current = run;
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
    const onFocus = () => void rescan.current(false);
    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") void rescan.current(false);
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
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
          <>
            {importsSkin
              ? "点“导入皮肤”，选中包含 skin.toml 的皮肤文件夹。文件夹名只能用小写字母、数字和 . _ -，同名皮肤会被替换。"
              : "把包含 skin.toml 的皮肤文件夹复制到下面的目录，回到这个窗口时会自动列出。"}
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
            {skins.catalog?.directory && (
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
