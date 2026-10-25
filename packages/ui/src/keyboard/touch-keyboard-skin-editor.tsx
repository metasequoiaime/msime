import { AiSkinGeneration } from "./ai-skin-generation";
import { fullScreenColumn, fullScreenSurface } from "../core/full-screen-style";
import { useState } from "react";
import { ErrorAlert } from "../core/error-alert";
import { useTouchKeyboardSkinLibrary } from "./use-touch-keyboard-skin-library";
import { useTouchKeyboardSkinEditorState } from "./use-touch-keyboard-skin-editor-state";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import type { PreviewKey } from "./keyboard-layouts";
import { readAndCompressTouchSkinPhoto } from "./touch-skin-images";
import {
  hasReadableSkinText,
  skinColor,
  touchKeyboardBackgroundPresets,
  touchKeyboardSkinTemplates,
  type AiSkinClient,
  type CustomSkinLibraryClient,
  type TouchKeyboardSkinDesign,
  type TouchSkinKeyMaterial,
  type TouchSkinKeyShape,
} from "./touch-keyboard-skin-design";
import type { CommunitySkinClient } from "../community/community-skin-model";
import * as skin from "./touch-skin-style";
import * as doc from "../settings/document-style";
import { SettingsPageHeader } from "../settings/settings-page-header";
import { ActionButton } from "../core/action-button";
import { FluentIcon } from "../core/fluent-icons";
import { GroupList, Row } from "../core/platform-controls";
import { SelectRow } from "../settings/select-row";
import { SliderRow } from "../settings/slider-row";
import { SwitchRow } from "../settings/switch-row";
import { CheckGlyph } from "../core/check-glyph";
import { ChevronGlyph } from "../core/chevron-glyph";
import { RgbColorRow } from "../settings/rgb-color-row";

type Category = "背景" | "按键" | "文本" | "设计" | "我的";

const percent = (value: number) => `${Math.round(value * 100)}%`;

export function TouchKeyboardSkinEditor({
  design,
  selected,
  theme,
  disabled,
  library,
  aiSkins,
  communitySkins,
  onChange,
  onUse,
  onClose,
  fullScreen = false,
  touchRows,
  phonePreview = false,
}: {
  design: TouchKeyboardSkinDesign;
  selected: boolean;
  theme: "dark" | "light";
  disabled?: boolean;
  library?: CustomSkinLibraryClient;
  aiSkins?: AiSkinClient;
  communitySkins?: CommunitySkinClient;
  onChange: (design: TouchKeyboardSkinDesign) => void;
  onUse: () => void;
  onClose: () => void;
  /** 作为独立视图呈现：覆盖整个设置界面，标题前带返回按钮，返回即 `onClose`，所以不再画「完成」。HarmonyOS 手机的皮肤页这样打开它，不在页面里就地展开。 */
  fullScreen?: boolean;
  /** HarmonyOS 手机按当前中文键盘绘制全屏预览。 */
  touchRows?: PreviewKey[][];
  /** Android/iOS 的就地编辑器也使用手机比例；HarmonyOS 全屏时由 `fullScreen` 覆盖。 */
  phonePreview?: boolean;
}) {
  const [category, setCategory] = useState<Category>("背景");
  const [photoError, setPhotoError] = useState("");
  const [aiGenerationOpen, setAiGenerationOpen] = useState(false);
  const {
    saved,
    busy: libraryBusy,
    notice: libraryNotice,
    setNotice: setLibraryNotice,
    nameEditor,
    skinName,
    setSkinName,
    beginCreate,
    beginRename,
    cancelName,
    submitName,
    confirmation,
    requestMutation,
    cancelConfirmation,
    confirmMutation,
  } = useTouchKeyboardSkinLibrary(library);
  const saveNamedDesign = async () => {
    const result = await submitName(design);
    if (!result) return;
    setCategory("我的");
    if (result === "created") onUse();
  };
  const {
    apply,
    patch,
    undo: stepBack,
    redo: stepForward,
    canUndo,
    canRedo,
    reset,
    optimizeContrast,
  } = useTouchKeyboardSkinEditorState({ design, onChange });

  const editor = (
    <div
      className={`${fullScreen ? fullScreenColumn : skin.editor} ${skin.editorFilled}`}
      aria-label="自定义皮肤编辑器"
    >
      {fullScreen ? (
        <>
          <SettingsPageHeader title="设计我的皮肤" onBack={onClose} level={2} />
          <p className={doc.generationNote}>修改后自动保存，键盘随即换上</p>
        </>
      ) : (
        <div className={skin.editorHeading}>
          <div className="section-title">
            自定义皮肤
            <small>修改后自动保存，键盘随即换上</small>
          </div>
          <ActionButton action={onClose} className="secondary" label="完成" />
        </div>
      )}
      {/* 预览在控件上方：改哪一项都能马上看到键盘的样子。全屏时整块吸顶。 */}
      <div className={skin.previewDock(fullScreen)}>
        <div className={skin.previewCard}>
          <div className={skin.previewFrame(fullScreen)}>
            <ScreenKeyboardPreview
              theme={theme}
              skin="custom"
              customDesign={design}
              phone={fullScreen || phonePreview}
              touchRows={touchRows}
            />
          </div>
          <div className={skin.previewBar}>
            <ActionButton
              action={stepBack}
              className={skin.iconButton}
              disabled={!canUndo || disabled}
              ariaLabel="撤销设计"
              label={<FluentIcon name="arrow_reset" />}
            />
            <ActionButton
              action={stepForward}
              className={skin.iconButton}
              disabled={!canRedo || disabled}
              ariaLabel="重做"
              label={<FluentIcon name="arrow_reset" className="-scale-x-100" />}
            />
            <span className={skin.previewBarSpacer} />
            {library && (
              <ActionButton
                action={beginCreate}
                className={skin.tonalButton}
                disabled={disabled || libraryBusy || saved.length >= 12}
                label="保存设计"
              />
            )}
            <ActionButton
              action={onUse}
              className="primary"
              disabled={disabled || selected}
              label={selected ? "正在使用" : "使用皮肤"}
            />
          </div>
        </div>
      </div>
      {aiGenerationOpen && aiSkins && library && (
        <AiSkinGeneration
          client={aiSkins}
          library={library}
          communitySkins={communitySkins}
          touchRows={touchRows}
          phonePreview={phonePreview}
          onUse={(design) => {
            apply(design);
            setLibraryNotice("AI 设计已载入；可以继续调整。保存页面设置后会应用到键盘。");
          }}
          onClose={() => setAiGenerationOpen(false)}
        />
      )}
      {nameEditor && (
        <div
          className={skin.libraryDialog}
          role="dialog"
          aria-label={nameEditor.operation === "create" ? "保存我的皮肤" : "重命名皮肤"}
        >
          <label>
            皮肤名称
            <input
              aria-label="皮肤名称"
              value={skinName}
              onChange={(event) => setSkinName(event.target.value)}
            />
          </label>
          <div>
            <ActionButton
              action={saveNamedDesign}
              className="primary"
              disabled={libraryBusy || !skinName.trim()}
              label={nameEditor.operation === "create" ? "确认保存" : "确认重命名"}
            />
            <ActionButton
              action={cancelName}
              className="secondary"
              disabled={libraryBusy}
              label="取消"
            />
          </div>
        </div>
      )}
      {confirmation && (
        <div
          className={skin.libraryDialog}
          role="alertdialog"
          aria-label={
            confirmation.operation === "update" ? "确认更新已保存皮肤" : "确认删除已保存皮肤"
          }
        >
          <p>
            {confirmation.operation === "update"
              ? `用当前设计更新“${confirmation.item.name}”？`
              : `删除“${confirmation.item.name}”？`}
          </p>
          <div>
            <ActionButton
              action={() => void confirmMutation(design)}
              className={confirmation.operation === "delete" ? "danger" : "primary"}
              disabled={libraryBusy}
              label={confirmation.operation === "update" ? "确认更新" : "确认删除"}
            />
            <ActionButton
              action={cancelConfirmation}
              className="secondary"
              disabled={libraryBusy}
              label="取消"
            />
          </div>
        </div>
      )}
      {libraryNotice && (
        <p className={skin.libraryNotice} role="status">
          {libraryNotice}
        </p>
      )}
      {aiSkins && library && (
        <ActionButton
          action={() => setAiGenerationOpen(true)}
          className={skin.aiEntry}
          disabled={disabled || libraryBusy}
          ariaLabel="AI 皮肤抽卡"
          label={
            <>
              <FluentIcon name="sparkle" size={24} />
              <span className={skin.aiEntryText}>
                <strong>AI 皮肤抽卡</strong>
                <small>一次抽三款原创皮肤，喜欢就留下</small>
              </span>
              <ChevronGlyph size={16} />
            </>
          }
        />
      )}
      <div className={skin.editorTabs} role="tablist" aria-label="皮肤编辑分类">
        {(
          ["背景", "按键", "文本", "设计", ...(library ? ["我的" as const] : [])] as Category[]
        ).map((item) => (
          <button
            type="button"
            role="tab"
            aria-selected={category === item}
            className={skin.editorTab(category === item)}
            onClick={() => setCategory(item)}
            key={item}
          >
            {item}
          </button>
        ))}
      </div>

      <div className={skin.editorControls}>
        {category === "背景" && (
          <>
            <GroupList title="预设">
              <div className={skin.groupBody}>
                <div className={skin.backgroundGrid}>
                  {touchKeyboardBackgroundPresets.map((preset) => {
                    const chosen =
                      design.photo === undefined &&
                      design.background === preset.start &&
                      design.gradientEnd === preset.end &&
                      design.keyBackground === preset.keyBackground;
                    return (
                      <button
                        type="button"
                        className={skin.backgroundPreset}
                        aria-label={`背景预设 ${preset.title}`}
                        aria-pressed={chosen}
                        onClick={() =>
                          patch({
                            photo: undefined,
                            background: preset.start,
                            gradientEnd: preset.end,
                            gradientHorizontal: false,
                            keyBackground: preset.keyBackground,
                            keyForeground: preset.keyForeground,
                            accent: preset.accent,
                            actionBackground: preset.actionBackground,
                          })
                        }
                        key={preset.title}
                      >
                        <span
                          className={skin.backgroundSwatch(chosen)}
                          style={{
                            background: `linear-gradient(135deg, ${skinColor(preset.start)}, ${skinColor(preset.end ?? preset.start)})`,
                          }}
                        >
                          {chosen && (
                            <span className={skin.backgroundCheck}>
                              <CheckGlyph />
                            </span>
                          )}
                        </span>
                        <span>{preset.title}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            </GroupList>
            <GroupList title="颜色">
              <RgbColorRow
                title={design.gradientEnd === undefined ? "背景颜色" : "渐变起始色"}
                value={design.background}
                onChange={(background) => patch({ background })}
              />
              <SwitchRow
                title="渐变背景"
                checked={design.gradientEnd !== undefined}
                onChange={(checked) =>
                  patch({ gradientEnd: checked ? design.background : undefined })
                }
              />
              <RgbColorRow
                title="渐变结束色"
                hidden={design.gradientEnd === undefined}
                value={design.gradientEnd ?? design.background}
                onChange={(gradientEnd) => patch({ gradientEnd })}
              />
              <SwitchRow
                title="横向渐变"
                hidden={design.gradientEnd === undefined}
                checked={design.gradientHorizontal ?? false}
                onChange={(checked) => patch({ gradientHorizontal: checked })}
              />
            </GroupList>
            <GroupList title="照片壁纸">
              <Row
                title="照片"
                description={design.photo ? "已设为键盘背景" : "用自己的照片做键盘背景"}
              >
                {design.photo && (
                  <button
                    type="button"
                    className={skin.rowActionDanger}
                    onClick={() => patch({ photo: undefined })}
                  >
                    移除
                  </button>
                )}
                <label className={skin.rowAction}>
                  {design.photo ? "更换照片" : "选择照片"}
                  <input
                    aria-label="选择皮肤照片"
                    type="file"
                    accept="image/*"
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      event.target.value = "";
                      if (!file) return;
                      setPhotoError("");
                      void readAndCompressTouchSkinPhoto(file)
                        .then((photo) => patch({ photo }))
                        .catch(() => setPhotoError("无法读取或压缩这张照片，请换一张再试。"));
                    }}
                  />
                </label>
              </Row>
              {photoError && <ErrorAlert className={skin.warning}>{photoError}</ErrorAlert>}
              <SliderRow
                title="照片位置"
                aria-label="照片位置"
                hidden={!design.photo}
                wide
                min={0}
                max={1}
                step={0.01}
                value={design.photoPosition ?? 0.5}
                displayValue={percent(design.photoPosition ?? 0.5)}
                onChange={(photoPosition) => patch({ photoPosition })}
              />
              <SliderRow
                title="压暗照片"
                aria-label="压暗照片"
                hidden={!design.photo}
                wide
                min={0}
                max={0.8}
                step={0.01}
                value={design.photoShade ?? 0.25}
                displayValue={percent(design.photoShade ?? 0.25)}
                onChange={(photoShade) => patch({ photoShade })}
              />
            </GroupList>
            <GroupList title="纹理">
              <SelectRow
                title="背景纹理"
                aria-label="背景纹理"
                value={design.pattern}
                onChange={(event) =>
                  patch({
                    pattern: Number(event.target.value) as TouchKeyboardSkinDesign["pattern"],
                  })
                }
              >
                <option value="0">纯色</option>
                <option value="1">网点</option>
                <option value="2">网格</option>
                <option value="3">波纹</option>
              </SelectRow>
              <SliderRow
                title="纹理强度"
                aria-label="纹理强度"
                hidden={design.pattern === 0}
                wide
                min={0}
                max={0.5}
                step={0.01}
                value={design.patternOpacity ?? 0.15}
                displayValue={percent(design.patternOpacity ?? 0.15)}
                onChange={(patternOpacity) => patch({ patternOpacity })}
              />
            </GroupList>
          </>
        )}

        {category === "按键" && (
          <>
            <GroupList title="颜色">
              <RgbColorRow
                title="键帽颜色"
                value={design.keyBackground}
                onChange={(keyBackground) => patch({ keyBackground })}
              />
              <RgbColorRow
                title="功能键颜色"
                value={design.actionBackground}
                onChange={(actionBackground) => patch({ actionBackground })}
              />
              <RgbColorRow
                title="边框颜色"
                value={design.customBorderColor ?? design.accent}
                onChange={(customBorderColor) => patch({ customBorderColor })}
              />
            </GroupList>
            <GroupList title="造型">
              <SelectRow
                title="键帽造型"
                aria-label="键帽造型"
                value={design.keyShape ?? "rounded"}
                onChange={(event) => patch({ keyShape: event.target.value as TouchSkinKeyShape })}
              >
                <option value="rounded">圆角</option>
                <option value="capsule">胶囊</option>
                <option value="ticket">票券</option>
                <option value="pebble">卵石</option>
              </SelectRow>
              <SelectRow
                title="键帽材质"
                aria-label="键帽材质"
                value={design.keyMaterial ?? "flat"}
                onChange={(event) =>
                  patch({ keyMaterial: event.target.value as TouchSkinKeyMaterial })
                }
              >
                <option value="flat">哑光</option>
                <option value="raised">立体</option>
                <option value="glass">玻璃</option>
                <option value="paper">纸张</option>
              </SelectRow>
              <SliderRow
                title="键帽不透明度"
                aria-label="键帽不透明度"
                wide
                min={0.25}
                max={1}
                step={0.01}
                value={design.keyOpacity ?? 1}
                displayValue={percent(design.keyOpacity ?? 1)}
                onChange={(keyOpacity) => patch({ keyOpacity })}
              />
              <SliderRow
                title="圆角"
                aria-label="自定义键帽圆角"
                wide
                min={0}
                max={20}
                step={1}
                value={design.cornerRadius}
                displayValue={Math.round(design.cornerRadius)}
                onChange={(cornerRadius) => patch({ cornerRadius })}
              />
              <SliderRow
                title="边框"
                aria-label="自定义键帽边框"
                wide
                min={0}
                max={2}
                step={0.5}
                value={design.borderWidth}
                displayValue={design.borderWidth.toFixed(1)}
                onChange={(borderWidth) => patch({ borderWidth })}
              />
              <SliderRow
                title="阴影"
                aria-label="自定义键帽阴影"
                wide
                min={0}
                max={0.4}
                step={0.05}
                value={design.shadow}
                displayValue={percent(design.shadow)}
                onChange={(shadow) => patch({ shadow })}
              />
            </GroupList>
          </>
        )}

        {category === "文本" && (
          <>
            <GroupList title="文字">
              <SwitchRow
                title="等宽字形"
                checked={design.monospaced}
                onChange={(monospaced) => patch({ monospaced })}
              />
              <RgbColorRow
                title="按键文字"
                value={design.keyForeground}
                onChange={(keyForeground) => patch({ keyForeground })}
              />
              <RgbColorRow
                title="提示与工具栏"
                value={design.accent}
                onChange={(accent) => patch({ accent })}
              />
              {!hasReadableSkinText(design) && (
                <p className={skin.warning}>部分文字与背景对比度偏低，建议调整配色。</p>
              )}
            </GroupList>
            <div className={skin.footerAction}>
              <ActionButton
                action={optimizeContrast}
                className={skin.tonalButton}
                label="优化文字对比度"
              />
            </div>
          </>
        )}

        {category === "设计" && (
          <>
            <GroupList title="模板">
              <div className={skin.groupBody}>
                <div className={skin.templateGrid}>
                  {touchKeyboardSkinTemplates.map((template) => (
                    <button
                      type="button"
                      className={skin.templateCard}
                      aria-label={`皮肤模板 ${template.title}`}
                      onClick={() => apply(template.design)}
                      key={template.title}
                      style={{
                        background: `linear-gradient(135deg, ${skinColor(template.design.background)}, ${skinColor(template.design.gradientEnd ?? template.design.background)})`,
                        color: skinColor(template.design.accent),
                      }}
                    >
                      <ScreenKeyboardPreview
                        theme={theme}
                        skin="custom"
                        customDesign={template.design}
                        compact
                      />
                      <span>{template.title}</span>
                    </button>
                  ))}
                </div>
              </div>
            </GroupList>
            <div className={skin.footerAction}>
              <ActionButton action={reset} className="danger-text" label="重置我的皮肤" />
            </div>
          </>
        )}
        {category === "我的" && library && (
          <GroupList title={`我的皮肤 · ${saved.length}/12`}>
            <div className={skin.groupBody} aria-label="我的皮肤图库">
              {libraryBusy && saved.length === 0 && <p className={skin.libraryEmpty}>正在读取…</p>}
              {!libraryBusy && saved.length === 0 && (
                <p className={skin.libraryEmpty}>
                  还没有命名保存的皮肤。调整满意后，点击“保存设计”。
                </p>
              )}
              <div className={skin.libraryList}>
                {saved.map((item) => (
                  <article key={item.id} className={skin.libraryCard}>
                    <button
                      type="button"
                      className={skin.libraryApply}
                      aria-label={`应用已保存皮肤 ${item.name}`}
                      onClick={() => {
                        apply(item.design);
                        setLibraryNotice(
                          "已载入设计；点击“使用皮肤”并保存页面设置后会应用到键盘。",
                        );
                      }}
                    >
                      <ScreenKeyboardPreview
                        theme={theme}
                        skin="custom"
                        customDesign={item.design}
                        compact
                      />
                      <strong>{item.name}</strong>
                    </button>
                    <div className={skin.libraryActions}>
                      <ActionButton
                        action={() => requestMutation("update", item)}
                        className="secondary"
                        disabled={libraryBusy}
                        ariaLabel={`用当前设计更新 ${item.name}`}
                        label="更新"
                      />
                      <ActionButton
                        action={() => beginRename(item)}
                        className="secondary"
                        disabled={libraryBusy}
                        ariaLabel={`重命名 ${item.name}`}
                        label="重命名"
                      />
                      <ActionButton
                        action={() => requestMutation("delete", item)}
                        className="danger-text"
                        disabled={libraryBusy}
                        ariaLabel={`删除 ${item.name}`}
                        label="删除"
                      />
                    </div>
                  </article>
                ))}
              </div>
            </div>
          </GroupList>
        )}
      </div>
    </div>
  );
  return fullScreen ? (
    <div className={fullScreenSurface} role="dialog" aria-modal="true" aria-label="设计我的皮肤">
      {editor}
    </div>
  ) : (
    editor
  );
}
