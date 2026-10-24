import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { GroupList, Row, Segmented, Select, Switch } from "../core/platform-controls";
import { ActionButton } from "../core/action-button";
import { ErrorAlert } from "../core/error-alert";
import { StatusMessage } from "../core/status-message";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import { bytesToBase64 } from "../core/base64";
import { SelectRow } from "../settings/select-row";
import { SliderRow } from "../settings/slider-row";
import { TextInputRow } from "../settings/text-input-row";
import { HexColorRow } from "../settings/hex-color-row";
import { SettingsTextareaField } from "../settings/settings-textarea-field";
import { SettingsGroupBlock } from "../settings/settings-group-block";
import * as settings from "../settings/settings-style";
import { themeEntry, type BaseGlobalTheme } from "../theme/global-theme";
import { notifySkinCatalogChanged } from "./skin-catalog-changes";
import { skinSlotLabel, type ExternalSkin, type SkinCatalog } from "./external-skins";
import type { SkinImageReader } from "./skin-image";
import {
  animationPresets,
  blankDraft,
  builtinBases,
  candidateSkinBaseTitle,
  checkSkinId,
  decorationMotions,
  deriveSkinId,
  draftColorKeys,
  draftColorTitles,
  draftFromSkin,
  draftFromTheme,
  draftProblems,
  drawnBase,
  fixedMode,
  installedSkins,
  isWindowsLook,
  livelyAnimation,
  lookTemplateManifest,
  normalizeCandidateSkinDraft,
  paletteContrast,
  windowsLooks,
  type BackgroundFit,
  type CandidateSkinBase,
  type CandidateSkinDraft,
  type CandidateSkinEditorClient,
  type DecorationAlign,
  type DraftColorKey,
  type DraftImage,
  type DraftLayout,
  type DraftMode,
  type DraftPalette,
} from "./candidate-skin-draft";
import {
  DraftImageError,
  isAnimatedImage,
  readDraftImage,
  sniffImageType,
} from "./candidate-skin-images";
import { buildCandidateSkinPackage, type PreviewRenderer } from "./candidate-skin-package";
import { candidateSkinSaveMessage } from "./candidate-skin-editor-messages";
import {
  CandidateSkinModePreview,
  useMotionReplay,
  usePrefersReducedMotion,
} from "./candidate-skin-editor-preview";

/** 选一张图片：按钮打开系统的文件选择器（隐藏的文件输入框只接收结果，键盘焦点停在按钮上）。 */
function ImageRow({
  title,
  label,
  description,
  image,
  disabled,
  onPick,
  onRemove,
}: {
  title: string;
  /** 按钮的名字里用的图片名，例如「背景图」。 */
  label: string;
  description: ReactNode;
  image: DraftImage | null;
  disabled: boolean;
  onPick: (file: File) => void;
  onRemove: () => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <Row
      title={title}
      description={
        <>
          {description}
          {image && (
            <span className={`block ${settings.skinEditorImageName}`}>
              {image.fileName} · {image.width}×{image.height} · {Math.ceil(image.byteLength / 1024)}{" "}
              KB
              {image.animated ? " · 动图" : ""}
            </span>
          )}
        </>
      }
    >
      <span className={settings.skinEditorActions}>
        {image && (
          <ActionButton
            action={onRemove}
            disabled={disabled}
            label="移除"
            ariaLabel={`移除${label}`}
          />
        )}
        <ActionButton
          action={() => input.current?.click()}
          disabled={disabled}
          label={image ? "更换图片" : "选择图片"}
          ariaLabel={`${image ? "更换" : "选择"}${label}`}
        />
        <input
          ref={input}
          type="file"
          hidden
          tabIndex={-1}
          aria-hidden="true"
          accept="image/png,image/jpeg,image/gif,image/webp,.png,.jpg,.jpeg,.gif,.webp,.apng"
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = "";
            if (file) onPick(file);
          }}
        />
      </span>
    </Row>
  );
}

/** 已安装皮肤的图片读进草稿，复制皮肤时用。 */
async function installedImage(
  read: SkinImageReader,
  skin: ExternalSkin,
  path: string | null | undefined,
): Promise<DraftImage | null> {
  if (!path) return null;
  const resource = await read(skin.id, path);
  const bytes = Uint8Array.from(resource.bytes);
  const type = sniffImageType(bytes);
  if (!type) return null;
  const data = bytesToBase64(bytes);
  const image = new Image();
  image.src = `data:${type};base64,${data}`;
  await image.decode();
  return {
    fileName: path,
    contentType: type,
    data,
    byteLength: bytes.length,
    width: image.naturalWidth,
    height: image.naturalHeight,
    animated: isAnimatedImage(bytes, type),
  };
}

type Saved = { id: string; name: string; base: BaseGlobalTheme; replaced: boolean };

export interface CandidateSkinEditorProps {
  client: CandidateSkinEditorClient;
  /** 本机皮肤目录的最新扫描：检查 ID 是否已被占用，也是「复制已安装的皮肤」的来源。 */
  catalog: SkinCatalog | null;
  readImage?: SkinImageReader;
  /** 把刚保存的皮肤放进它所属的明暗槽位并启用，与「本地皮肤」卡片的开关相同。 */
  onUse: (id: string, base: BaseGlobalTheme) => void;
  /** 这个 ID 是否已在浅色或深色槽位里使用。 */
  inUse: (id: string) => boolean;
  /** 切到「本地皮肤」标签。 */
  onShowLocal: () => void;
  /** 一份要载入的草稿（例如以后的 AI 方案）；每次换成新的对象就重新载入一次。 */
  initialDraft?: CandidateSkinDraft;
  renderPreview?: PreviewRenderer;
}

/**
 * 桌面主题页「制作皮肤」标签：从空白、内置主题、Windows 外观或已安装的皮肤开始，编辑候选窗皮肤并保存到本机皮肤目录。
 *
 * 编辑的是一份草稿（`CandidateSkinDraft`），预览随草稿实时更新；保存时画出预览图、写出 `skin.toml`，交给宿主经共享安装器装入，之后它出现在「本地皮肤」里。替换已有的同 ID 皮肤之前一定先问。
 */
export function CandidateSkinEditor({
  client,
  catalog,
  readImage,
  onUse,
  inUse,
  onShowLocal,
  initialDraft,
  renderPreview,
}: CandidateSkinEditorProps) {
  const [draft, setDraft] = useState<CandidateSkinDraft>(() =>
    initialDraft ? normalizeCandidateSkinDraft(initialDraft) : blankDraft(),
  );
  const [pristine, setPristine] = useState(true);
  useEffect(() => {
    if (!initialDraft) return;
    setDraft(normalizeCandidateSkinDraft(initialDraft));
    setPristine(true);
  }, [initialDraft]);
  const [source, setSource] = useState("blank");
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState("");
  const [startNotice, setStartNotice] = useState("");
  const [editingMode, setEditingMode] = useState<DraftMode>("light");
  const [orientation, setOrientation] = useState<DraftLayout>("horizontal");
  const [imageErrors, setImageErrors] = useState<
    Partial<Record<"background" | "decoration", string>>
  >({});
  const [ownId, setOwnId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [saved, setSaved] = useState<Saved | null>(null);
  const [showProblems, setShowProblems] = useState(false);
  const { confirm, confirmation } = useConfirm();
  const reducedMotion = usePrefersReducedMotion();
  const { phase, play } = useMotionReplay(draft, reducedMotion);
  const installed = useMemo(() => installedSkins(catalog), [catalog]);
  const taken = useMemo(
    () => new Set([...installed.keys()].filter((id) => id !== ownId)),
    [installed, ownId],
  );
  const idCheck = checkSkinId(draft.id, installed, ownId);
  const problems = draftProblems(draft, installed, ownId);
  const fixed = fixedMode(draft.base);
  const modes: DraftMode[] = fixed ? [fixed] : draft.modes;
  const shownMode: DraftMode = modes.includes(editingMode) ? editingMode : (modes[0] ?? "light");

  /** 改草稿。`derive` 时按新名称重新推导没被手改过的 ID。 */
  const update = (change: (current: CandidateSkinDraft) => CandidateSkinDraft) => {
    setPristine(false);
    setSaved(null);
    setSaveError("");
    setDraft((current) => change(current));
  };
  const patch = (value: Partial<CandidateSkinDraft>) =>
    update((current) => ({ ...current, ...value }));
  const patchPalette = (mode: DraftMode, value: Partial<DraftPalette>) =>
    update((current) => ({
      ...current,
      palettes: { ...current.palettes, [mode]: { ...current.palettes[mode], ...value } },
    }));

  const load = (next: CandidateSkinDraft, notice = "") => {
    const named = next.name.trim();
    setDraft({
      ...next,
      id: next.idEdited ? next.id : named ? deriveSkinId(named, taken) : "",
    });
    setPristine(true);
    setSaved(null);
    setSaveError("");
    setShowProblems(false);
    setOwnId(null);
    setStartNotice(notice);
    setImageErrors({});
  };

  const start = async () => {
    if (starting) return;
    if (!pristine) {
      const ok = await confirm({
        title: "换一个起点？",
        message: "当前的修改还没有保存，换一个起点会丢掉它们。",
        confirmLabel: "换起点",
        danger: true,
      });
      if (!ok) return;
    }
    setStartError("");
    const [kind, id = ""] = source.split(":");
    if (kind === "blank") return load(blankDraft());
    if (kind === "theme") return load(draftFromTheme(id as BaseGlobalTheme));
    setStarting(true);
    try {
      if (kind === "look" && isWindowsLook(id)) {
        // Windows 外观的配色只在 client-services 里有一份，按装皮肤的规则让宿主补齐。
        const template = await client.check({
          packageId: "template",
          manifest: lookTemplateManifest(id),
          files: {},
        });
        const next = draftFromSkin(template, {}, id);
        load({
          ...next,
          name: "",
          description: "",
          carried: { ...next.carried, version: "1.0", author: null, license: null },
        });
      } else if (kind === "skin") {
        const skin = catalog?.packages.find((entry) => entry.id === id);
        if (!skin) throw new Error("missing skin");
        const decorationPath =
          skin.decorationTopDip > 0 && skin.decorationWidthDip > 0
            ? skin.decorationImage === undefined
              ? skin.preview
              : skin.decorationImage
            : null;
        const [decoration, background] = readImage
          ? await Promise.all([
              installedImage(readImage, skin, decorationPath),
              installedImage(readImage, skin, skin.background?.image),
            ])
          : [null, null];
        const next = draftFromSkin(skin, { decoration, background });
        const notices = [
          "已复制这款皮肤的配色、图片和动效；编辑器不支持的布局细节（内边距、字体、阴影等）不会带过来。",
        ];
        if (skin.canvas)
          notices.push("原皮肤用的是全贴图（canvas）底，编辑器不支持贴图面板，只保留了配色。");
        if ((decorationPath || skin.background) && !readImage)
          notices.push("当前宿主读不出皮肤图片，图片没有复制。");
        load(
          {
            ...next,
            name: `${skin.name} 副本`.slice(0, 40),
            id: deriveSkinId(`${skin.id}-copy`, taken),
            idEdited: true,
          },
          notices.join(""),
        );
      }
    } catch {
      setStartError(
        kind === "look"
          ? "无法读取这个外观的配色，请重试。"
          : "无法读取这款皮肤，请刷新本地皮肤后重试。",
      );
    } finally {
      setStarting(false);
    }
  };

  const pickImage = (slot: "background" | "decoration", file: File) => {
    setImageErrors((current) => ({ ...current, [slot]: "" }));
    void readDraftImage(file).then(
      (image) =>
        update((current) =>
          slot === "background"
            ? {
                ...current,
                background: {
                  image,
                  fit: current.background?.fit ?? "cover",
                  opacity: current.background?.opacity ?? 1,
                },
              }
            : {
                ...current,
                decoration: current.decoration
                  ? { ...current.decoration, image }
                  : {
                      image,
                      topInset: 24,
                      width: Math.max(
                        1,
                        Math.min(160, Math.round((image.width / image.height) * 40) || 96),
                      ),
                      align: "right",
                      motion: "none",
                      motionPeriodMs: 3000,
                    },
              },
        ),
      (error: unknown) =>
        setImageErrors((current) => ({
          ...current,
          [slot]:
            error instanceof DraftImageError ? error.message : "无法读取这张图片，请换一张再试。",
        })),
    );
  };

  const save = async (replace = false): Promise<void> => {
    if (saving) return;
    if (problems.length) {
      setShowProblems(true);
      return;
    }
    setSaving(true);
    setSaveError("");
    try {
      const request = await buildCandidateSkinPackage(draft, renderPreview);
      const created = await client.create(request, replace || draft.id === ownId);
      setOwnId(created.id);
      setSaved({
        id: created.id,
        name: draft.name.trim(),
        base: drawnBase(draft.base),
        replaced: created.replaced,
      });
      setPristine(true);
      notifySkinCatalogChanged();
    } catch (error) {
      if (errorCode(error) === "candidate_skin_exists" && !replace) {
        setSaving(false);
        const existing = installed.get(draft.id);
        const ok = await confirm({
          title: "替换已有的皮肤？",
          message: `本地已有 ID 为“${draft.id}”的皮肤${existing ? `「${existing}」` : ""}。保存会整体替换它，原来的配色和图片都不再保留。`,
          confirmLabel: "替换",
          danger: true,
        });
        if (ok) return save(true);
        return;
      }
      setSaveError(candidateSkinSaveMessage(error));
    } finally {
      setSaving(false);
    }
  };

  const palette = draft.palettes[shownMode];
  const contrast = paletteContrast(palette, shownMode);
  const lowContrast = contrast.filter((reading) => !reading.ok);
  const problemFor = (field: string) =>
    showProblems ? problems.find((problem) => problem.field === field)?.message : undefined;
  const decoration = draft.decoration;
  const background = draft.background;
  const schema2Note = "打开后皮肤写成第 2 版清单，较早版本的输入法读不了它。";

  return (
    <div className="flex flex-col gap-6">
      <section
        className={settings.skinEditorPreview(orientation === "horizontal")}
        aria-label="实时预览"
      >
        <div className={settings.skinEditorPreviewBar}>
          <Segmented
            aria-label="预览的候选排列"
            options={[
              { value: "horizontal", label: "横排" },
              { value: "vertical", label: "竖排" },
            ]}
            value={orientation}
            onChange={setOrientation}
          />
          <ActionButton
            action={play}
            disabled={!draft.animation.enabled || reducedMotion}
            label="播放动效"
          />
        </div>
        <div className={settings.skinEditorPreviewModes}>
          {(["light", "dark"] as const).map((mode) => (
            <CandidateSkinModePreview
              key={mode}
              draft={draft}
              mode={mode}
              orientation={orientation}
              phase={phase}
              reducedMotion={reducedMotion}
            />
          ))}
        </div>
        {reducedMotion &&
          (draft.animation.enabled || (decoration && decoration.motion !== "none")) && (
            <StatusMessage className={settings.skinCardDescription}>
              系统开启了「减少动态效果」，预览不播放动效；各平台的候选窗同样不会播放。
            </StatusMessage>
          )}
      </section>

      <GroupList title="起点">
        <Row title="从这里开始" description="载入会用它的配色和设置重新填写下面的内容。">
          <span className={settings.skinEditorActions}>
            <Select value={source} onChange={(event) => setSource(event.target.value)}>
              <option value="blank">空白皮肤</option>
              <optgroup label="内置主题">
                {builtinBases.map((id) => (
                  <option key={id} value={`theme:${id}`}>
                    {themeEntry(id).title}
                  </option>
                ))}
              </optgroup>
              <optgroup label="Windows 外观">
                {windowsLooks.map((look) => (
                  <option key={look.id} value={`look:${look.id}`}>
                    {look.title}
                  </option>
                ))}
              </optgroup>
              {!!catalog?.packages.length && (
                <optgroup label="复制已安装的皮肤">
                  {catalog.packages.map((skin) => (
                    <option key={skin.id} value={`skin:${skin.id}`}>
                      {skin.name}
                    </option>
                  ))}
                </optgroup>
              )}
            </Select>
            <ActionButton
              action={start}
              disabled={starting || saving}
              label={starting ? "正在读取…" : "载入"}
            />
          </span>
        </Row>
        {(startError || startNotice) && (
          <SettingsGroupBlock>
            {startError && <ErrorAlert>{startError}</ErrorAlert>}
            {startNotice && (
              <StatusMessage className={settings.skinCardDescription}>{startNotice}</StatusMessage>
            )}
          </SettingsGroupBlock>
        )}
      </GroupList>

      <GroupList title="基本信息">
        <TextInputRow
          title="名称"
          description={problemFor("name") ?? "显示在本地皮肤的卡片上。"}
          label="皮肤名称"
          placeholder="例如：秋日暖阳"
          value={draft.name}
          onChange={(name) =>
            update((current) => ({
              ...current,
              name,
              id: current.idEdited
                ? current.id
                : name.trim()
                  ? deriveSkinId(name.trim(), taken)
                  : "",
            }))
          }
        />
        <TextInputRow
          title="皮肤 ID"
          description={
            idCheck.kind === "ok" || (!draft.id && !showProblems)
              ? draft.idEdited
                ? "皮肤文件夹的名字，只能用小写英文字母、数字和 . _ -。"
                : "按名称自动生成；也可以自己改。"
              : idCheck.message
          }
          label="皮肤 ID"
          aria-invalid={idCheck.kind === "error" && (draft.id !== "" || showProblems)}
          spellCheck={false}
          autoCapitalize="off"
          value={draft.id}
          onChange={(id) => patch({ id: id.trim(), idEdited: id.trim() !== "" })}
        />
        <SettingsGroupBlock>
          <SettingsTextareaField
            label="说明"
            description={problemFor("description")}
            ariaLabel="皮肤说明"
            rows={2}
            value={draft.description}
            onChange={(description) => patch({ description })}
          />
        </SettingsGroupBlock>
        <SelectRow
          title="底色"
          description={`没设的颜色和圆角跟随它。${skinSlotLabel(drawnBase(draft.base))}。`}
          value={draft.base}
          onChange={(event) => {
            const base = event.target.value as CandidateSkinBase;
            const mode = fixedMode(base);
            patch({
              base,
              modes: mode ? [mode] : draft.modes.length ? draft.modes : ["light", "dark"],
            });
          }}
        >
          <optgroup label="内置主题">
            {builtinBases.map((id) => (
              <option key={id} value={id}>
                {themeEntry(id).title}
              </option>
            ))}
          </optgroup>
          <optgroup label="Windows 外观">
            {windowsLooks.map((look) => (
              <option key={look.id} value={look.id}>
                {look.title}
              </option>
            ))}
          </optgroup>
        </SelectRow>
        <Row title="候选排列" description={problemFor("layouts") ?? "皮肤用在哪些候选排列下。"}>
          <span className={settings.skinEditorActions}>
            {(
              [
                ["horizontal", "横排"],
                ["vertical", "竖排"],
              ] as const
            ).map(([layout, label]) => (
              <label key={layout} className="check-option">
                <input
                  type="checkbox"
                  className="platform-check"
                  checked={draft.layouts.includes(layout)}
                  onChange={(event) =>
                    patch({
                      layouts: (["horizontal", "vertical"] as const).filter((value) =>
                        value === layout ? event.target.checked : draft.layouts.includes(value),
                      ),
                    })
                  }
                />
                {label}
              </label>
            ))}
          </span>
        </Row>
        <Row
          title="明暗模式"
          description={
            problemFor("modes") ??
            (fixed
              ? `底色「${candidateSkinBaseTitle(draft.base)}」是${fixed === "dark" ? "深色" : "浅色"}主题，皮肤只用在${fixed === "dark" ? "深色" : "浅色"}模式。`
              : "皮肤用在哪些明暗模式下，每种都有自己的配色。")
          }
        >
          <span className={settings.skinEditorActions}>
            {(
              [
                ["light", "浅色"],
                ["dark", "深色"],
              ] as const
            ).map(([mode, label]) => (
              <label key={mode} className="check-option">
                <input
                  type="checkbox"
                  className="platform-check"
                  disabled={fixed !== null}
                  checked={modes.includes(mode)}
                  onChange={(event) =>
                    patch({
                      modes: (["light", "dark"] as const).filter((value) =>
                        value === mode ? event.target.checked : draft.modes.includes(value),
                      ),
                    })
                  }
                />
                {label}
              </label>
            ))}
          </span>
        </Row>
      </GroupList>

      {modes.length > 0 && (
        <GroupList title="配色">
          {modes.length > 1 && (
            <Row title="正在编辑" description="浅色和深色模式各有一套配色。">
              <Segmented
                options={[
                  { value: "light", label: "浅色" },
                  { value: "dark", label: "深色" },
                ]}
                value={shownMode}
                onChange={setEditingMode}
              />
            </Row>
          )}
          {draftColorKeys.map((key: DraftColorKey) => (
            <HexColorRow
              key={`${shownMode}-${key}`}
              title={`${draftColorTitles[key]}（${shownMode === "dark" ? "深色" : "浅色"}）`}
              value={palette[key]}
              onChange={(value) => patchPalette(shownMode, { [key]: value })}
            />
          ))}
          <Row title="选中竖条" description="选中的候选前画一条强调色竖条。">
            <Switch
              checked={palette.showSelectedBar}
              onChange={(showSelectedBar) => patchPalette(shownMode, { showSelectedBar })}
            />
          </Row>
          <SettingsGroupBlock>
            <p
              className={`m-0 mb-2 text-[13px] ${lowContrast.length ? "text-danger" : "[color:var(--p-sub)]"}`}
            >
              {lowContrast.length
                ? `有 ${lowContrast.length} 组文字对比度低于 4.5:1，可能看不清。`
                : "文字对比度都不低于 4.5:1。"}
            </p>
            <dl
              className={settings.skinEditorContrast}
              aria-label={`${shownMode === "dark" ? "深色" : "浅色"}配色的文字对比度`}
            >
              {contrast.map((reading) => (
                <div key={reading.label} className="contents">
                  <dt>{reading.label}</dt>
                  <dd className={settings.skinEditorContrastValue(reading.ok)}>
                    {reading.ratio.toFixed(1)}:1{reading.ok ? "" : " 偏低"}
                  </dd>
                </div>
              ))}
            </dl>
          </SettingsGroupBlock>
        </GroupList>
      )}

      <GroupList title="窗口">
        <Row
          title="自定义圆角"
          description="关闭时用输入法自己的圆角。「候选窗口」页设了圆角时以那里为准。"
        >
          <Switch
            checked={draft.cornerRadius !== null}
            onChange={(on) => patch({ cornerRadius: on ? 8 : null })}
          />
        </Row>
        <SliderRow
          title="圆角"
          hidden={draft.cornerRadius === null}
          min={0}
          max={32}
          value={draft.cornerRadius ?? 8}
          displayValue={`${draft.cornerRadius ?? 8} px`}
          valueText={`${draft.cornerRadius ?? 8} 像素`}
          onChange={(cornerRadius) => patch({ cornerRadius })}
        />
        <SliderRow
          title="最小宽度"
          description="候选少时窗口也不会比它窄；0 表示随内容。"
          min={0}
          max={600}
          step={10}
          value={Math.min(draft.minWidth, 600)}
          displayValue={`${draft.minWidth} px`}
          valueText={`${draft.minWidth} 像素`}
          onChange={(minWidth) => patch({ minWidth })}
        />
      </GroupList>

      <GroupList title="背景图">
        <ImageRow
          title="图片"
          label="背景图"
          description="画在窗口底色之上、候选之下，按窗口圆角裁剪，浅色和深色模式共用。PNG、JPEG、GIF 或 WebP，不超过 1 MB。上面的文字对比度不计背景图，换图后请看预览确认文字清楚。"
          image={background?.image ?? null}
          disabled={saving}
          onPick={(file) => pickImage("background", file)}
          onRemove={() => patch({ background: null })}
        />
        {imageErrors.background && <ErrorAlert>{imageErrors.background}</ErrorAlert>}
        <SelectRow
          title="填充方式"
          hidden={!background}
          value={background?.fit ?? "cover"}
          onChange={(event) =>
            background &&
            patch({ background: { ...background, fit: event.target.value as BackgroundFit } })
          }
        >
          <option value="cover">铺满（裁掉多余部分）</option>
          <option value="contain">完整显示</option>
          <option value="stretch">拉伸</option>
        </SelectRow>
        <SliderRow
          title="不透明度"
          hidden={!background}
          min={0}
          max={100}
          value={Math.round((background?.opacity ?? 1) * 100)}
          displayValue={`${Math.round((background?.opacity ?? 1) * 100)}%`}
          onChange={(value) =>
            background && patch({ background: { ...background, opacity: value / 100 } })
          }
        />
      </GroupList>

      <GroupList title="装饰图">
        <ImageRow
          title="图片"
          label="装饰图"
          description="候选窗上方的一张小图，例如吉祥物，底部压在窗口上沿。可以是 APNG 动图。"
          image={decoration?.image ?? null}
          disabled={saving}
          onPick={(file) => pickImage("decoration", file)}
          onRemove={() => patch({ decoration: null })}
        />
        {imageErrors.decoration && <ErrorAlert>{imageErrors.decoration}</ErrorAlert>}
        <SliderRow
          title="顶部留白"
          description="窗口上方给装饰图留的高度。"
          hidden={!decoration}
          min={1}
          max={200}
          value={Math.min(decoration?.topInset ?? 24, 200)}
          displayValue={`${decoration?.topInset ?? 24} px`}
          onChange={(topInset) => decoration && patch({ decoration: { ...decoration, topInset } })}
        />
        <SliderRow
          title="装饰图宽度"
          hidden={!decoration}
          min={1}
          max={400}
          value={Math.min(decoration?.width ?? 96, 400)}
          displayValue={`${decoration?.width ?? 96} px`}
          onChange={(width) => decoration && patch({ decoration: { ...decoration, width } })}
        />
        <SelectRow
          title="对齐"
          hidden={!decoration}
          value={decoration?.align ?? "right"}
          onChange={(event) =>
            decoration &&
            patch({ decoration: { ...decoration, align: event.target.value as DecorationAlign } })
          }
        >
          <option value="left">靠左</option>
          <option value="center">居中</option>
          <option value="right">靠右</option>
        </SelectRow>
        <SelectRow
          title="待机动效"
          description={`候选窗在屏幕上时装饰图的轻微动作。${schema2Note}`}
          hidden={!decoration}
          value={decoration?.motion ?? "none"}
          onChange={(event) =>
            decoration &&
            patch({
              decoration: { ...decoration, motion: event.target.value as typeof decoration.motion },
            })
          }
        >
          {decorationMotions.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </SelectRow>
        <SliderRow
          title="动效周期"
          hidden={!decoration || decoration.motion === "none"}
          min={1200}
          max={8000}
          step={100}
          value={decoration?.motionPeriodMs ?? 3000}
          displayValue={`${((decoration?.motionPeriodMs ?? 3000) / 1000).toFixed(1)} 秒`}
          onChange={(motionPeriodMs) =>
            decoration && patch({ decoration: { ...decoration, motionPeriodMs } })
          }
        />
      </GroupList>

      <GroupList title="候选窗动效">
        <Row title="启用动效" description={`候选窗出现、消失和选中时的动画。${schema2Note}`}>
          <Switch
            checked={draft.animation.enabled}
            onChange={(enabled) =>
              patch({
                animation: enabled
                  ? { ...livelyAnimation, ...draft.animation, enabled }
                  : { ...draft.animation, enabled },
              })
            }
          />
        </Row>
        {(
          [
            ["enter", "出现"],
            ["exit", "消失"],
            ["selection", "选中"],
          ] as const
        ).map(([key, title]) => (
          <SelectRow
            key={key}
            title={title}
            hidden={!draft.animation.enabled}
            value={draft.animation[key]}
            onChange={(event) =>
              patch({
                animation: {
                  ...draft.animation,
                  [key]: event.target.value as typeof draft.animation.enter,
                },
              })
            }
          >
            {animationPresets.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </SelectRow>
        ))}
        <SliderRow
          title="时长"
          hidden={!draft.animation.enabled}
          min={40}
          max={600}
          step={10}
          value={draft.animation.durationMs}
          displayValue={`${draft.animation.durationMs} 毫秒`}
          onChange={(durationMs) => patch({ animation: { ...draft.animation, durationMs } })}
        />
        <SliderRow
          title="逐行间隔"
          description="出现时每行候选比上一行晚多久。"
          hidden={!draft.animation.enabled}
          min={0}
          max={80}
          value={draft.animation.staggerMs}
          displayValue={`${draft.animation.staggerMs} 毫秒`}
          onChange={(staggerMs) => patch({ animation: { ...draft.animation, staggerMs } })}
        />
      </GroupList>

      <GroupList title="保存">
        <SettingsGroupBlock>
          {showProblems && problems.length > 0 && (
            <div role="alert">
              <p className="m-0 mb-1 text-[13px] text-danger">保存之前还要改这些：</p>
              <ul className={settings.skinEditorProblems}>
                {problems.map((problem) => (
                  <li key={`${problem.field}-${problem.message}`}>{problem.message}</li>
                ))}
              </ul>
            </div>
          )}
          {saveError && <ErrorAlert>{saveError}</ErrorAlert>}
          {saved && (
            <StatusMessage role="status" className={settings.skinCardDescription}>
              {saved.replaced ? "已替换" : "已保存"}「{saved.name}」，它已出现在「本地皮肤」中。
              {inUse(saved.id) ? `正在使用：${skinSlotLabel(saved.base)}。` : ""}
            </StatusMessage>
          )}
          <span className={`${settings.skinEditorActions} mt-2`}>
            <ActionButton
              action={() => save()}
              className="primary"
              disabled={saving}
              ariaBusy={saving}
              label={saving ? "正在保存…" : ownId && draft.id === ownId ? "保存修改" : "保存皮肤"}
            />
            {saved && !inUse(saved.id) && (
              <ActionButton action={() => onUse(saved.id, saved.base)} label="立即使用" />
            )}
            {saved && <ActionButton action={onShowLocal} label="在本地皮肤中查看" />}
          </span>
        </SettingsGroupBlock>
      </GroupList>
      {confirmation}
    </div>
  );
}
