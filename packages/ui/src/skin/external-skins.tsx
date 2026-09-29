import { useEffect, useId, useRef, useState, type CSSProperties } from "react";
import { SkinCandidatePreview } from "./skin-candidate-preview";
import { SkinToolbarPreview } from "./skin-toolbar-preview";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import type { SkinFontReader } from "./skin-font";
import { installSkinPalette } from "./skin-palette";
import { useToolbarCss, type ToolbarCssReader } from "./use-toolbar-css";
import * as settings from "../settings/settings-style";
import {
  customCandidateStyle,
  themeEntry,
  type GlobalTheme,
  type PackageCandidatePalette,
} from "../theme/global-theme";

type Palette = PackageCandidatePalette;
export type ExternalSkin = {
  id: string;
  name: string;
  version: string;
  /** The global theme under the package's own colours; `system` for none. The manifest parser never gives `custom`. */
  base: Exclude<GlobalTheme, "custom">;
  author: string | null;
  description: string | null;
  layouts: string[];
  themes: string[];
  minWidthDip: number;
  decorationTopDip: number;
  decorationWidthDip: number;
  toolbarStylesheet: string | null;
  preview: string | null;
  candidate: { dark: Palette; light: Palette };
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

/** The package palette `resolve()` draws for `theme`: the declared one, or none, in which case the base is drawn alone. A package is never layered over its other mode. */
export function drawnPackagePalette(
  skin: Pick<ExternalSkin, "themes" | "candidate">,
  theme: "dark" | "light",
): Palette | null {
  return skin.themes.includes(theme) ? skin.candidate[theme] : null;
}

/** The one package switch the `--cand-*` properties cannot carry: hiding the selection bar. Colours are never written as rules; they go through `customCandidateStyle`, layered as `resolve()` layers them. */
export function selectedBarCss(scope: string, palette: Palette | null): string[] {
  return palette?.showSelectedBar === false
    ? [`.${scope} .first::before{display:none !important}`]
    : [];
}

function ExternalSkinCard({
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
}: {
  skin: ExternalSkin;
  selected: string;
  layout: string;
  /** Choosing a package passes its manifest base, which the custom theme is then drawn over. */
  onSelect: (id: string, base: ExternalSkin["base"]) => void;
  readImage?: SkinImageReader;
  readFont?: SkinFontReader;
  readToolbarCss?: ToolbarCssReader;
  revision: number;
  activeTheme: "dark" | "light";
  toolbarPreview: boolean;
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
  const [paletteFailed, setPaletteFailed] = useState(false);
  const palette = drawnPackagePalette(skin, theme);
  const hideBar = palette?.showSelectedBar === false;
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
  // Card-only overrides must not change runtime compatibility or selection. A package over a built-in base is drawn in that base's mode, so the host mode does not rule it out.
  const compatible =
    skin.layouts.includes(layout) && (fixed !== null || skin.themes.includes(activeTheme));
  const top = dimension(skin.decorationTopDip, 500);
  const width = dimension(skin.decorationWidthDip, 1000);
  const decorated = top > 0 && width > 0;
  const image = useSkinImage(readImage, skin.id, decorated ? skin.preview : null, revision);
  const [decodeFailed, setDecodeFailed] = useState(false);
  useEffect(() => setDecodeFailed(false), [image]);
  const geometry = {
    // The package drawn over its base theme, as `resolve()` layers them; a card has no pickers.
    ...customCandidateStyle(skin.base, undefined, palette),
    "--msime-skin-min-width": `${dimension(skin.minWidthDip, 1000)}px`,
    "--msime-skin-decoration-top": `${decorated ? top : 0}px`,
    "--msime-skin-decoration-width": `${decorated ? width : 0}px`,
  } as CSSProperties;
  return (
    <article
      aria-label={skin.name}
      className={`skin-card${selected === skin.id ? " selected" : ""}${decorated ? " external-skin-decorated" : ""}`}
    >
      <div className="skin-card-header">
        <div className="skin-card-body">
          <span className="skin-card-title">{skin.name}</span>
          <span className="external-skin-meta">
            {[skin.id, skin.version && `v${skin.version}`, skin.author].filter(Boolean).join(" · ")}
          </span>
          <span className="skin-card-description">
            {compatible
              ? skin.description || `基于 ${themeEntry(skin.base).title}`
              : `当前布局或明暗模式不受支持（${skin.layouts.join("/")}，${skin.themes.join("/")}）`}
          </span>
        </div>
        <div className="skin-card-actions">
          <button
            type="button"
            role="switch"
            aria-label={skin.name}
            aria-checked={selected === skin.id}
            disabled={!compatible}
            className="skin-selection-switch"
            onClick={() => onSelect(skin.id, skin.base)}
          >
            <span />
          </button>
          {fixed === null && (
            <button
              type="button"
              className="skin-preview-switch"
              onClick={() => setOverride(theme === "dark" ? "light" : "dark")}
            >
              {theme === "dark" ? "预览浅色" : "预览深色"}
            </button>
          )}
        </div>
      </div>
      <div
        data-skin-preview=""
        className={`${settings.skinCardPreview} ${scope}${theme === "light" ? " theme-light" : ""}`}
        style={geometry}
        data-preview-theme={theme}
        aria-hidden="true"
      >
        <div className={settings.skinPreviewStage} data-skin-stage="">
          <SkinCandidatePreview
            orientation="horizontal"
            decorated={decorated}
            image={decodeFailed ? undefined : image?.url}
            onImageError={() => setDecodeFailed(true)}
          />
        </div>
        <div className={settings.skinPreviewStage} data-skin-stage="">
          <SkinCandidatePreview
            orientation="vertical"
            decorated={decorated}
            image={decodeFailed ? undefined : image?.url}
            onImageError={() => setDecodeFailed(true)}
          />
        </div>
        {toolbarPreview && (
          <div className={settings.skinPreviewStage} data-skin-stage="">
            <SkinToolbarPreview />
          </div>
        )}
      </div>
      {paletteFailed && (
        <p role="status" className="skin-card-description external-skin-resource-note">
          当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。
        </p>
      )}
      {(image?.failed || decodeFailed) && (
        <p role="status" className="skin-card-description external-skin-resource-note">
          皮肤图片加载失败，保留基础预览。可刷新皮肤重试。
        </p>
      )}
      {skin.preview && decorated && !readImage && (
        <p className="skin-card-description external-skin-resource-note">
          当前宿主不支持皮肤图片预览。
        </p>
      )}
      {toolbarPreview && skin.toolbarStylesheet && !readToolbarCss && (
        <p className="skin-card-description external-skin-resource-note">
          当前宿主不支持外部工具栏样式。
        </p>
      )}
      {toolbarState === "failed" && (
        <p role="status">工具栏样式加载失败，保留基础预览。可刷新皮肤重试。</p>
      )}
      {toolbarState === "partial" && (
        <p role="status">已应用工具栏基础样式；关联资源及部分规则尚未支持。</p>
      )}
    </article>
  );
}

export function ExternalSkins({
  scan,
  openDirectory,
  importsSkin = false,
  readImage,
  readFont,
  readToolbarCss,
  selected,
  layout,
  onSelect,
  activeTheme = "dark",
  toolbarPreview = true,
}: {
  scan?: () => Promise<SkinCatalog>;
  openDirectory?: () => Promise<void>;
  /**
   * The host copies a skin the user points at, instead of opening a folder for them to drop one
   * into. Its skin folder is inside an application sandbox, so there is nothing to open — the
   * button has to say what it actually does, or it promises a folder that never appears.
   */
  importsSkin?: boolean;
  readImage?: SkinImageReader;
  readFont?: SkinFontReader;
  readToolbarCss?: ToolbarCssReader;
  selected: string;
  layout: string;
  /** Choosing a package passes its manifest base, which the custom theme is then drawn over. */
  onSelect: (id: string, base: ExternalSkin["base"]) => void;
  activeTheme?: "dark" | "light";
  /** The host draws a floating toolbar the skin styles. The Linux hosts present the toolbar as an input method menu, so their cards preview only the candidate window. */
  toolbarPreview?: boolean;
}) {
  const [catalog, setCatalog] = useState<SkinCatalog | null>(null);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);
  const pending = useRef(false);
  const [opening, setOpening] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const openGeneration = useRef(0);
  const openPending = useRef(false);
  useEffect(() => {
    openGeneration.current++;
    openPending.current = false;
    setOpening(false);
    setOpenFailed(false);
    return () => {
      openGeneration.current++;
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
    generation.current++;
    pending.current = false;
    setCatalog(null);
    setBusy(false);
    setFailed(false);
    return () => {
      generation.current++;
      pending.current = false;
    };
  }, [scan]);
  async function refresh() {
    if (!scan || pending.current) return;
    pending.current = true;
    const current = ++generation.current;
    setBusy(true);
    setFailed(false);
    try {
      const result = await scan();
      if (current === generation.current) {
        setCatalog(result);
        setRevision((value) => value + 1);
      }
    } catch {
      if (current === generation.current) setFailed(true);
    } finally {
      if (current === generation.current) {
        pending.current = false;
        setBusy(false);
      }
    }
  }
  return (
    <section aria-label="外部皮肤" className="external-skins">
      <div className="external-skin-heading">
        <div>
          <div className="section-title">外部皮肤</div>
          <p className="skin-card-description">
            {importsSkin
              ? "点“导入皮肤”，选中包含 skin.toml 的皮肤文件夹。文件夹名只能用小写字母、数字和 . _ -，同名皮肤会被替换。"
              : "把包含 skin.toml 的皮肤文件夹复制到下面的目录，然后刷新。"}
          </p>
          <code className="external-skin-directory">
            {catalog?.directory || "扫描后显示客户端皮肤目录"}
          </code>
        </div>
        <div className="external-skin-actions">
          <button
            type="button"
            className="skin-preview-switch"
            disabled={!openDirectory || opening}
            onClick={() => void openFolder()}
          >
            {opening
              ? importsSkin
                ? "正在导入…"
                : "正在打开…"
              : importsSkin
                ? "导入皮肤"
                : "打开目录"}
          </button>
          <button
            type="button"
            className="skin-preview-switch"
            disabled={!scan || busy}
            onClick={() => void refresh()}
          >
            {busy ? "正在扫描…" : "刷新皮肤"}
          </button>
        </div>
      </div>
      {openFailed && (
        <p role="alert">{importsSkin ? "导入皮肤失败，请重试。" : "无法打开皮肤目录，请重试。"}</p>
      )}
      {failed && <p role="alert">读取皮肤目录失败，请重试。{catalog && "仍显示上次扫描结果。"}</p>}
      <div role="status">
        {!scan
          ? "当前宿主不支持扫描外部皮肤。"
          : busy
            ? "正在读取皮肤目录。"
            : !catalog
              ? "尚未扫描。点击“刷新皮肤”读取皮肤目录。"
              : !catalog.packages.length
                ? "没有发现外部皮肤。"
                : ""}
      </div>
      <div className="skin-grid">
        {catalog?.packages.map((skin) => (
          <ExternalSkinCard
            key={skin.id}
            skin={skin}
            selected={selected}
            layout={layout}
            onSelect={onSelect}
            readImage={readImage}
            readFont={readFont}
            readToolbarCss={readToolbarCss}
            revision={revision}
            activeTheme={activeTheme}
            toolbarPreview={toolbarPreview}
          />
        ))}
      </div>
      {!!catalog?.issues.length && (
        <details className="external-skin-diagnostics">
          <summary>已忽略 {catalog.issues.length} 个无效皮肤目录</summary>
          <ul>
            {catalog.issues.map((issue, index) => (
              <li key={index}>
                {issue.folder}：{issue.reason}
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
