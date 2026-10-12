import { ControlledCheckbox } from "../core/controlled-checkbox";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { StatusMessage } from "../core/status-message";
import { ErrorAlert } from "../core/error-alert";
import { useAsyncActionRunner } from "../core/use-async-action";
import { aiSkinMessage, libraryError } from "./touch-keyboard-skin-errors";
import { createAiSkinPrompt } from "./touch-keyboard-skin-ai";
import { randomRequestId } from "../core/random-id";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import { boundTouchSkinArtwork, readAndCompressTouchSkinPhoto } from "./touch-skin-images";
import {
  defaultTouchKeyboardSkinDesign,
  boundedSkinName,
  hasReadableSkinText,
  normalizeTouchKeyboardSkinDesign,
  readableSkinText,
  skinColor,
  skinColorNumber,
  skinContrast,
  touchKeyboardBackgroundPresets,
  touchKeyboardSkinTemplates,
  updateSavedTouchKeyboardSkinName,
  type AiSkinClient,
  type AiSkinProposal,
  type CustomSkinLibraryAction,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
  type TouchKeyboardSkinDesign,
  type TouchSkinKeyMaterial,
  type TouchSkinKeyShape,
} from "./touch-keyboard-skin-design";
import type { CommunitySkinClient } from "../community/community-skins";
import {
  communitySkinCategories,
  communitySkinCategoryLabels,
  type CommunitySkinCategory,
} from "../community/community-skin-category";
import * as skin from "./touch-skin-style";
import * as doc from "../settings/document-style";
import * as community from "../community/community-style";
import { SettingsPageHeader } from "../settings/settings-page-header";
import { ActionButton } from "../core/action-button";
import { FluentIcon } from "../core/fluent-icons";
import { GroupList, Row } from "../core/platform-controls";
import { SelectRow } from "../settings/select-row";
import { SliderRow } from "../settings/slider-row";
import { SwitchRow } from "../settings/switch-row";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";

type Category = "背景" | "按键" | "文本" | "设计" | "我的";
type NameEditor = { operation: "create" } | { operation: "rename"; id: string };
type Confirmation = { operation: "update" | "delete"; item: SavedTouchKeyboardSkin };

// 全屏呈现以页面自身的背景覆盖整个设置界面，并避开系统栏。功能页里打开的社区图库（`CommunityGallerySubview`）也用这一套。
export const fullScreenSurface =
  "fixed inset-0 z-40 overflow-y-auto bg-[var(--p-bg)] pt-[env(safe-area-inset-top,0px)] pb-[env(safe-area-inset-bottom,0px)] text-[var(--p-text)] [font-family:var(--p-font)]";
export const fullScreenColumn = "mx-auto flex w-full max-w-[760px] flex-col gap-3.5 px-4 pt-1 pb-6";

/**
 * AI 皮肤抽卡：每次抽三款皮肤，每款都可以使用、保存到「我的皮肤」或发布。编辑器以对话框形式在页面上打开它。`fullScreen` 则把它呈现为独立视图，标题前带返回按钮，手机皮肤网格里的 AI 磁贴就是这样打开它的。`useLabel` 是把方案交给 `onUse` 的那个按钮的名称。
 */
export function AiSkinGeneration({
  client,
  library,
  communitySkins,
  fullScreen = false,
  useLabel = "使用并继续编辑",
  onUse,
  onClose,
}: {
  client: AiSkinClient;
  library: CustomSkinLibraryClient;
  communitySkins?: CommunitySkinClient;
  fullScreen?: boolean;
  useLabel?: string;
  onUse: (design: TouchKeyboardSkinDesign) => void;
  onClose: () => void;
}) {
  const [proposals, setProposals] = useState<AiSkinProposal[]>([]);
  const [busy, setBusy] = useState(false);
  const [completed, setCompleted] = useState(0);
  const [message, setMessage] = useState("");
  const [requestId, setRequestId] = useState("");
  const [saved, setSaved] = useState<Record<string, SavedTouchKeyboardSkin>>({});
  const [publishing, setPublishing] = useState<SavedTouchKeyboardSkin | null>(null);
  const [publishDescription, setPublishDescription] = useState("");
  const [publishAgreed, setPublishAgreed] = useState(false);
  const [publishCategory, setPublishCategory] = useState<CommunitySkinCategory>("other");
  const [publishBusy, setPublishBusy] = useState(false);
  const publishRunning = useRef(false);
  const surface = useRef<HTMLElement>(null);
  // 全屏视图叠在皮肤页上，共用外壳的滚动容器：不滚回顶部的话，它会停在皮肤页原来的滚动位置，标题和返回按钮都在屏幕外。
  useLayoutEffect(() => {
    if (fullScreen) surface.current?.scrollIntoView?.({ block: "start" });
  }, [fullScreen]);
  const generateRunning = useRef(false);
  const saveRunning = useRef(false);
  const requestRef = useRef("");
  const mounted = useMountedRef();
  const progressGeneration = useAsyncGeneration(client, requestId);

  useEffect(() => {
    const generation = progressGeneration.current;
    requestRef.current = requestId;
    if (!client.onProgress) return;
    let unsubscribe: (() => void) | undefined;
    void client
      .onProgress((progress) => {
        if (generation === progressGeneration.current && progress.requestId === requestRef.current)
          setCompleted(progress.completed);
      })
      .then((value) => {
        if (generation === progressGeneration.current) unsubscribe = value;
        else value();
      });
    return () => {
      unsubscribe?.();
    };
  }, [client, progressGeneration, requestId]);

  useEffect(
    () => () => {
      if (requestRef.current) void client.cancel(requestRef.current).catch(() => undefined);
    },
    [client],
  );

  const cancelGeneration = () => {
    const id = requestRef.current;
    if (!id) return;
    // 先使本地请求代次失效；服务端取消可能在生成结果之后才返回，迟到的结果不能重新出现在页面上。
    requestRef.current = "";
    setRequestId("");
    generateRunning.current = false;
    setBusy(false);
    setMessage("已取消生成");
    void client.cancel(id).catch(() => undefined);
  };

  const generate = async () => {
    if (busy || generateRunning.current) return;
    generateRunning.current = true;
    const id = randomRequestId("ai-skin");
    requestRef.current = id;
    setRequestId(id);
    setBusy(true);
    setCompleted(0);
    setMessage("");
    try {
      const result = await client.generate(id, createAiSkinPrompt());
      const prepared = await Promise.all(
        result.map(async (proposal) => ({
          ...proposal,
          design: {
            ...proposal.design,
            photo: await boundTouchSkinArtwork(proposal.artwork),
            photoShade: 0.08,
            photoPosition: 0.5,
            keyOpacity: 0.92,
            pattern: 0 as const,
          },
        })),
      );
      if (!mounted.current || requestRef.current !== id) return;
      setProposals(prepared);
    } catch (error) {
      if (requestRef.current === id && errorCode(error) !== "ai_skin_cancelled")
        if (mounted.current) setMessage(aiSkinMessage(error));
    } finally {
      if (requestRef.current === id) {
        generateRunning.current = false;
        requestRef.current = "";
        if (mounted.current) setBusy(false);
      }
    }
  };

  const save = async (proposal: AiSkinProposal): Promise<SavedTouchKeyboardSkin | null> => {
    const existing = saved[proposal.name];
    if (existing || saveRunning.current) return existing ?? null;
    saveRunning.current = true;
    try {
      const current = await library.load();
      if (current.length >= 12) throw { code: "custom_skin_full" };
      let name = proposal.name;
      let suffix = 2;
      while (current.some((item) => item.name === name)) {
        name = `${proposal.name.slice(0, 26)} ${suffix}`;
        suffix += 1;
      }
      const next = await library.mutate({ operation: "create", name, design: proposal.design });
      const item = next.find((value) => value.name === name);
      if (!item) throw new Error("skin was not saved");
      if (!mounted.current) return null;
      setSaved((currentSaved) => ({ ...currentSaved, [proposal.name]: item }));
      setMessage("已保存到“我的皮肤”。");
      return item;
    } catch (error) {
      if (mounted.current) setMessage(libraryError(error));
      return null;
    } finally {
      saveRunning.current = false;
    }
  };

  const publish = async () => {
    if (!publishing || !communitySkins || !publishAgreed || publishBusy || publishRunning.current)
      return;
    const name = publishing.name.trim();
    const description = publishDescription.trim();
    if (!name || name.length > 32 || description.length > 280) {
      setMessage("请填写有效的名称和设计说明。");
      return;
    }
    publishRunning.current = true;
    setPublishBusy(true);
    try {
      await communitySkins.publish(
        publishing.id,
        name,
        description,
        publishing.design,
        publishCategory,
      );
      if (!mounted.current) return;
      setPublishing(null);
      setPublishDescription("");
      setPublishAgreed(false);
      setPublishCategory("other");
      setMessage("已发布到社区。");
    } catch (error) {
      const code = errorCode(error);
      if (mounted.current)
        setMessage(code ? `发布失败：${code}` : "暂时无法发布皮肤，请稍后重试。");
    } finally {
      publishRunning.current = false;
      if (mounted.current) setPublishBusy(false);
    }
  };

  return (
    <div className={fullScreen ? fullScreenSurface : community.backdrop}>
      <section
        ref={surface}
        className={fullScreen ? fullScreenColumn : `${community.dialog} ${doc.generation}`}
        role="dialog"
        aria-modal="true"
        aria-label="AI 皮肤抽卡"
      >
        {fullScreen ? (
          <>
            <SettingsPageHeader
              title="AI 皮肤抽卡"
              onBack={onClose}
              backDisabled={busy}
              level={2}
            />
            <p className={doc.generationNote}>一次抽出三张原创皮肤，遇到喜欢的就留下。</p>
          </>
        ) : (
          <div className={community.dialogHeading}>
            <div>
              <h2>AI 皮肤抽卡</h2>
              <p>一次抽出三张原创皮肤，遇到喜欢的就留下。</p>
            </div>
            <ActionButton
              action={onClose}
              className={community.dialogClose}
              disabled={busy}
              ariaLabel="关闭 AI 皮肤抽卡"
              label="×"
            />
          </div>
        )}
        {proposals.length === 0 && (
          <div className={doc.mysteryCards} aria-hidden="true">
            {["leaf", "moon", "sparkles"].map((icon, index) => (
              <div key={icon} className={doc.mysteryCard(index)}>
                MSIME<span>{icon === "leaf" ? "♧" : icon === "moon" ? "☾" : "✦"}</span>等待揭晓
              </div>
            ))}
          </div>
        )}
        <ActionButton
          action={() => generate()}
          // 全屏时这是整页唯一的主操作，铺满宽度。
          className={fullScreen ? "primary w-full py-3 text-[15px]" : "primary"}
          disabled={busy}
          ariaLabel="抽三张皮肤"
          label={proposals.length ? "再抽三张" : "抽三张皮肤"}
        />
        <p className={doc.generationNote}>
          AI 随机搭配插画、键帽造型与材质。抽到的皮肤可以继续编辑、保存或分享。
        </p>
        {busy && (
          <StatusMessage role="status">
            主题插画已完成 {completed}/3，可能需要几分钟…{" "}
            <ActionButton action={cancelGeneration} className="secondary" label="取消" />
          </StatusMessage>
        )}
        {message && <StatusMessage role="status">{message}</StatusMessage>}
        <div className={doc.cardList}>
          {proposals.map((proposal) => {
            const item = saved[proposal.name];
            return (
              <article className={doc.card} key={proposal.name}>
                <h3>{proposal.name}</h3>
                <p>{proposal.description}</p>
                <ScreenKeyboardPreview theme="light" skin="custom" customDesign={proposal.design} />
                <div className={doc.cardActions}>
                  <ActionButton
                    action={() => {
                      onUse(proposal.design);
                      onClose();
                    }}
                    className="primary"
                    label={useLabel}
                  />
                  <ActionButton
                    action={async () => {
                      await save(proposal);
                    }}
                    className="secondary"
                    disabled={Boolean(item)}
                    label={item ? "已保存" : "保存到我的皮肤"}
                  />
                  {communitySkins && (
                    <ActionButton
                      action={async () => {
                        const value = await save(proposal);
                        if (value) {
                          setPublishing(value);
                          setPublishDescription(proposal.description);
                        }
                      }}
                      className="secondary"
                      label="发布到社区"
                    />
                  )}
                </div>
              </article>
            );
          })}
        </div>
        {publishing && communitySkins && (
          <div className={doc.publishForm} role="dialog" aria-label="发布 AI 皮肤">
            <h3>发布到社区</h3>
            <label>
              皮肤名称
              <input
                aria-label="AI 皮肤名称"
                value={publishing.name}
                maxLength={32}
                onChange={(event) =>
                  setPublishing((current) =>
                    current
                      ? updateSavedTouchKeyboardSkinName(current, event.target.value)
                      : current,
                  )
                }
              />
            </label>
            <label>
              设计说明
              <textarea
                aria-label="AI 皮肤说明"
                value={publishDescription}
                maxLength={280}
                onChange={(event) => setPublishDescription(event.target.value)}
              />
            </label>
            <label>
              分类
              <select
                aria-label="发布分类"
                value={publishCategory}
                disabled={publishBusy}
                onChange={(event) =>
                  setPublishCategory(event.target.value as CommunitySkinCategory)
                }
              >
                {communitySkinCategories.map((item) => (
                  <option key={item} value={item}>
                    {communitySkinCategoryLabels[item]}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <ControlledCheckbox checked={publishAgreed} onChange={setPublishAgreed} />
              我拥有发布所用素材的权利，并同意其他用户免费下载使用
            </label>
            <p>发布后插画背景将公开，请勿包含私人或敏感资料。</p>
            <ActionButton
              action={() => publish()}
              className="primary"
              disabled={!publishAgreed || publishBusy}
              label="公开发布"
            />
            <ActionButton
              action={() => setPublishing(null)}
              className="secondary"
              disabled={publishBusy}
              label="取消"
            />
          </div>
        )}
        {/* 全屏时标题前已有返回按钮，再放一个「完成」是重复的出口。 */}
        {!fullScreen && (
          <div className={community.dialogActions}>
            <ActionButton action={onClose} className="secondary" disabled={busy} label="完成" />
          </div>
        )}
      </section>
    </div>
  );
}

/** 颜色设置行：末端是当前色值和一个圆形色块，色块就是原生取色器，点开即系统的取色面板。 */
function ColorRow({
  title,
  value,
  hidden,
  onChange,
}: {
  title: string;
  value: number;
  hidden?: boolean;
  onChange: (value: number) => void;
}) {
  const hex = skinColor(value);
  return (
    <Row title={title} hidden={hidden}>
      <span className={skin.colorControl}>
        <span className={skin.colorValue} aria-hidden="true">
          {hex}
        </span>
        <input
          className={skin.colorSwatch}
          aria-label={title}
          type="color"
          value={hex}
          onChange={(event) => onChange(skinColorNumber(event.target.value))}
        />
      </span>
    </Row>
  );
}

function CheckGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </svg>
  );
}

function ChevronGlyph() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="m9 6 6 6-6 6" />
    </svg>
  );
}

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
}) {
  const [category, setCategory] = useState<Category>("背景");
  const [undo, setUndo] = useState<TouchKeyboardSkinDesign[]>([]);
  const [redo, setRedo] = useState<TouchKeyboardSkinDesign[]>([]);
  const [photoError, setPhotoError] = useState("");
  const [saved, setSaved] = useState<SavedTouchKeyboardSkin[]>([]);
  const [libraryNotice, setLibraryNotice] = useState("");
  const [nameEditor, setNameEditor] = useState<NameEditor | null>(null);
  const [skinName, setSkinName] = useState("");
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [aiGenerationOpen, setAiGenerationOpen] = useState(false);
  const {
    busy: libraryBusy,
    running: libraryActionBusy,
    run: runLibraryAction,
  } = useAsyncActionRunner(setLibraryNotice, undefined, library);
  useEffect(() => {
    if (!library) return;
    void runLibraryAction(
      async (isCurrent) => {
        const items = await library.load();
        if (isCurrent()) setSaved(items);
      },
      { formatError: libraryError },
    );
  }, [library, runLibraryAction]);
  const apply = (next: TouchKeyboardSkinDesign, record = true) => {
    const normalized = normalizeTouchKeyboardSkinDesign(next);
    if (JSON.stringify(normalized) === JSON.stringify(design)) return;
    if (record) {
      setUndo((items) => [...items, design].slice(-30));
      setRedo([]);
    }
    onChange(normalized);
  };
  const patch = (value: Partial<TouchKeyboardSkinDesign>, record = true) =>
    apply({ ...design, ...value }, record);
  const stepBack = () => {
    const next = undo.at(-1);
    if (!next) return;
    setUndo((items) => items.slice(0, -1));
    setRedo((items) => [...items, design].slice(-30));
    apply(next, false);
  };
  const stepForward = () => {
    const next = redo.at(-1);
    if (!next) return;
    setRedo((items) => items.slice(0, -1));
    setUndo((items) => [...items, design].slice(-30));
    apply(next, false);
  };
  const optimizeContrast = () => {
    const black = Math.min(
      skinContrast(0, design.background),
      skinContrast(0, design.keyBackground),
    );
    const white = Math.min(
      skinContrast(0xffffff, design.background),
      skinContrast(0xffffff, design.keyBackground),
    );
    patch({
      keyForeground: readableSkinText(design.keyBackground),
      accent: black >= white ? 0 : 0xffffff,
    });
  };
  const mutateLibrary = async (action: CustomSkinLibraryAction, success: string) => {
    if (!library || libraryActionBusy.current) return false;
    let succeeded = false;
    await runLibraryAction(
      async (isCurrent) => {
        const items = await library.mutate(action);
        if (!isCurrent()) return;
        setSaved(items);
        setLibraryNotice(success);
        succeeded = true;
      },
      { formatError: libraryError },
    );
    return succeeded;
  };
  const submitName = async () => {
    if (!nameEditor) return;
    const name = skinName.trim();
    if (!name) {
      setLibraryNotice("请输入皮肤名称。");
      return;
    }
    const created = nameEditor.operation === "create";
    const action: CustomSkinLibraryAction = created
      ? { operation: "create", name, design }
      : { operation: "rename", id: nameEditor.id, name };
    if (
      await mutateLibrary(
        action,
        created ? "设计已保存到我的皮肤；保存页面设置后会应用到键盘。" : "皮肤名称已更新。",
      )
    ) {
      setNameEditor(null);
      setCategory("我的");
      if (created) onUse();
    }
  };
  const confirmLibraryMutation = async () => {
    if (!confirmation) return;
    const { operation, item } = confirmation;
    const action: CustomSkinLibraryAction =
      operation === "update"
        ? { operation: "update", id: item.id, design }
        : { operation: "delete", id: item.id };
    if (
      await mutateLibrary(
        action,
        operation === "update" ? "已用当前设计更新这套皮肤。" : "已删除这套皮肤。",
      )
    )
      setConfirmation(null);
  };

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
              phone={fullScreen}
            />
          </div>
          <div className={skin.previewBar}>
            <ActionButton
              action={stepBack}
              className={skin.iconButton}
              disabled={!undo.length || disabled}
              ariaLabel="撤销设计"
              label={<FluentIcon name="arrow_reset" />}
            />
            <ActionButton
              action={stepForward}
              className={skin.iconButton}
              disabled={!redo.length || disabled}
              ariaLabel="重做"
              label={<FluentIcon name="arrow_reset" className="-scale-x-100" />}
            />
            <span className={skin.previewBarSpacer} />
            {library && (
              <ActionButton
                action={() => {
                  setSkinName(`我的设计 ${saved.length + 1}`);
                  setNameEditor({ operation: "create" });
                  setLibraryNotice("");
                }}
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
              onChange={(event) => setSkinName(boundedSkinName(event.target.value))}
            />
          </label>
          <div>
            <ActionButton
              action={() => submitName()}
              className="primary"
              disabled={libraryBusy || !skinName.trim()}
              label={nameEditor.operation === "create" ? "确认保存" : "确认重命名"}
            />
            <ActionButton
              action={() => setNameEditor(null)}
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
              action={() => confirmLibraryMutation()}
              className={confirmation.operation === "delete" ? "danger" : "primary"}
              disabled={libraryBusy}
              label={confirmation.operation === "update" ? "确认更新" : "确认删除"}
            />
            <ActionButton
              action={() => setConfirmation(null)}
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
              <ChevronGlyph />
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
              <ColorRow
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
              <ColorRow
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
              <ColorRow
                title="键帽颜色"
                value={design.keyBackground}
                onChange={(keyBackground) => patch({ keyBackground })}
              />
              <ColorRow
                title="功能键颜色"
                value={design.actionBackground}
                onChange={(actionBackground) => patch({ actionBackground })}
              />
              <ColorRow
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
              <ColorRow
                title="按键文字"
                value={design.keyForeground}
                onChange={(keyForeground) => patch({ keyForeground })}
              />
              <ColorRow
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
              <ActionButton
                action={() => apply(defaultTouchKeyboardSkinDesign)}
                className="danger-text"
                label="重置我的皮肤"
              />
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
                        action={() => setConfirmation({ operation: "update", item })}
                        className="secondary"
                        disabled={libraryBusy}
                        ariaLabel={`用当前设计更新 ${item.name}`}
                        label="更新"
                      />
                      <ActionButton
                        action={() => {
                          setSkinName(item.name);
                          setNameEditor({ operation: "rename", id: item.id });
                          setLibraryNotice("");
                        }}
                        className="secondary"
                        disabled={libraryBusy}
                        ariaLabel={`重命名 ${item.name}`}
                        label="重命名"
                      />
                      <ActionButton
                        action={() => setConfirmation({ operation: "delete", item })}
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
