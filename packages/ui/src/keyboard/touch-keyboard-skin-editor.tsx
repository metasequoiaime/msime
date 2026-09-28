import { useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { randomRequestId } from "../core/random-id";
import { truncateGraphemes } from "../core/text";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import {
  defaultTouchKeyboardSkinDesign,
  hasReadableSkinText,
  normalizeTouchKeyboardSkinDesign,
  readableSkinText,
  skinColor,
  skinContrast,
  touchKeyboardBackgroundPresets,
  touchKeyboardSkinTemplates,
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
import * as skin from "./touch-skin-style";
import * as doc from "../settings/document-style";
import * as community from "../community/community-style";

type Category = "背景" | "按键" | "文本" | "设计" | "我的";
type NameEditor = { operation: "create" } | { operation: "rename"; id: string };
type Confirmation = { operation: "update" | "delete"; item: SavedTouchKeyboardSkin };

function colorNumber(value: string): number {
  return Number.parseInt(value.slice(1), 16);
}

async function boundedPhoto(file: File): Promise<string> {
  if (!file.type.startsWith("image/") || file.size > 20_000_000) throw new Error("invalid image");
  // A data: URL, not a blob: one: the settings CSP is `img-src 'self' data:`, which WebKitGTK
  // enforces, so a blob: source never loads there and every photo read as undecodable.
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("image read failed"));
    reader.readAsDataURL(file);
  });
  const image = new Image();
  image.src = url;
  await new Promise<void>((resolve, reject) => {
    image.onload = () => resolve();
    image.onerror = () => reject(new Error("image decode failed"));
  });
  if (!image.naturalWidth || !image.naturalHeight) throw new Error("empty image");
  const scale = Math.min(1, 1024 / Math.max(image.naturalWidth, image.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
  canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("canvas unavailable");
  context.drawImage(image, 0, 0, canvas.width, canvas.height);
  for (const quality of [0.8, 0.6, 0.4, 0.2]) {
    const data = canvas.toDataURL("image/jpeg", quality).split(",")[1] ?? "";
    if (Math.floor(data.length * 0.75) <= 512_000) return data;
  }
  throw new Error("image too large");
}

async function boundedArtwork(artwork: AiSkinProposal["artwork"]): Promise<string> {
  const source = artwork.b64_json;
  const bytes = Uint8Array.from(atob(source), (character) => character.charCodeAt(0));
  if (bytes.length > 512_000) {
    const image = new Image();
    image.src = `data:${artwork.mime_type};base64,${source}`;
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error("artwork decode failed"));
    });
    const scale = Math.min(1, 1024 / Math.max(image.naturalWidth, image.naturalHeight));
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale));
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale));
    const context = canvas.getContext("2d");
    if (!context) throw new Error("canvas unavailable");
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    for (const quality of [0.8, 0.6, 0.4, 0.2]) {
      const result = canvas.toDataURL("image/jpeg", quality).split(",")[1] ?? "";
      if (Math.floor(result.length * 0.75) <= 512_000) return result;
    }
    throw new Error("artwork too large");
  }
  return source;
}

function aiSkinPrompt(): string {
  const scenes = [
    "月光森林里的狐狸茶屋",
    "云朵之间的鲸鱼邮局",
    "雨夜街角的猫咪书店",
    "星际列车上的花园",
    "蘑菇村的秋日集市",
    "珊瑚海里的水母舞会",
    "雪山小屋与极光",
    "竹林里的熊猫茶会",
    "沙漠星空下的旅店",
    "复古街机里的糖果世界",
    "樱花河畔的兔子野餐",
    "漂浮岛屿上的灯塔",
  ];
  const selected = [...scenes]
    .sort(() => Math.random() - 0.5)
    .slice(0, 3)
    .join("；");
  return `这是一次随机皮肤抽卡。分别围绕以下三个灵感创作三套主题，每套对应一个场景：${selected}。自由设计原创角色、插画风格和配色，三套键帽造型与材质都要不同，文字清晰。不要使用已有品牌或角色。`;
}

function aiSkinMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "ai_skin_invalid":
      return "AI 返回的皮肤设计或插画格式无效，请重新抽取。";
    case "ai_skin_cancelled":
      return "已取消这次抽卡。";
    case "ai_skin_busy":
      return "已有一次抽卡正在进行，请稍候。";
    case "account_unauthorized":
      return "请先登录，再来抽取皮肤。";
    case "account_rate_limited":
      return "请求过于频繁，请稍后再试。";
    default:
      return "AI 皮肤暂时不可用，请稍后重试。";
  }
}

function AiSkinGeneration({
  client,
  library,
  communitySkins,
  onUse,
  onClose,
}: {
  client: AiSkinClient;
  library: CustomSkinLibraryClient;
  communitySkins?: CommunitySkinClient;
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
  const [publishBusy, setPublishBusy] = useState(false);
  const requestRef = useRef("");
  const mounted = useRef(true);

  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );

  useEffect(() => {
    requestRef.current = requestId;
    if (!client.onProgress) return;
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void client
      .onProgress((progress) => {
        if (active && progress.requestId === requestRef.current) setCompleted(progress.completed);
      })
      .then((value) => {
        if (active) unsubscribe = value;
        else value();
      });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [client, requestId]);

  useEffect(
    () => () => {
      if (requestRef.current && busy) void client.cancel(requestRef.current).catch(() => undefined);
    },
    [busy, client],
  );

  const generate = async () => {
    if (busy) return;
    const id = randomRequestId("ai-skin");
    requestRef.current = id;
    setRequestId(id);
    setBusy(true);
    setCompleted(0);
    setMessage("");
    try {
      const result = await client.generate(id, aiSkinPrompt());
      const prepared = await Promise.all(
        result.map(async (proposal) => ({
          ...proposal,
          design: {
            ...proposal.design,
            photo: await boundedArtwork(proposal.artwork),
            photoShade: 0.08,
            photoPosition: 0.5,
            keyOpacity: 0.92,
            pattern: 0 as const,
          },
        })),
      );
      if (!mounted.current) return;
      setProposals(prepared);
    } catch (error) {
      if (errorCode(error) !== "ai_skin_cancelled")
        if (mounted.current) setMessage(aiSkinMessage(error));
    } finally {
      if (mounted.current) setBusy(false);
      if (requestRef.current === id) requestRef.current = "";
    }
  };

  const save = async (proposal: AiSkinProposal): Promise<SavedTouchKeyboardSkin | null> => {
    const existing = saved[proposal.name];
    if (existing) return existing;
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
    }
  };

  const publish = async () => {
    if (!publishing || !communitySkins || !publishAgreed || publishBusy) return;
    const name = publishing.name.trim();
    const description = publishDescription.trim();
    if (!name || name.length > 32 || description.length > 280) {
      setMessage("请填写有效的名称和设计说明。");
      return;
    }
    setPublishBusy(true);
    try {
      await communitySkins.publish(publishing.id, name, description, publishing.design);
      if (!mounted.current) return;
      setPublishing(null);
      setPublishDescription("");
      setPublishAgreed(false);
      setMessage("已发布到社区。");
    } catch (error) {
      const code = errorCode(error);
      if (mounted.current)
        setMessage(code ? `发布失败：${code}` : "暂时无法发布皮肤，请稍后重试。");
    } finally {
      if (mounted.current) setPublishBusy(false);
    }
  };

  return (
    <div className={community.backdrop}>
      <section
        className={`${community.dialog} ${doc.generation}`}
        role="dialog"
        aria-modal="true"
        aria-label="AI 皮肤抽卡"
      >
        <div className={community.dialogHeading}>
          <div>
            <h2>AI 皮肤抽卡</h2>
            <p>一次抽出三张原创皮肤，遇到喜欢的就留下。</p>
          </div>
          <button
            type="button"
            className={community.dialogClose}
            disabled={busy}
            onClick={onClose}
            aria-label="关闭 AI 皮肤抽卡"
          >
            ×
          </button>
        </div>
        {proposals.length === 0 && (
          <div className={doc.mysteryCards} aria-hidden="true">
            {["leaf", "moon", "sparkles"].map((icon, index) => (
              <div key={icon} className={doc.mysteryCard(index)}>
                MSIME<span>{icon === "leaf" ? "♧" : icon === "moon" ? "☾" : "✦"}</span>等待揭晓
              </div>
            ))}
          </div>
        )}
        <button
          type="button"
          className="primary"
          disabled={busy}
          onClick={() => void generate()}
          aria-label="抽三张皮肤"
        >
          {proposals.length ? "再抽三张" : "抽三张皮肤"}
        </button>
        <p className={doc.generationNote}>
          AI 随机搭配插画、键帽造型与材质。抽到的皮肤可以继续编辑、保存或分享。
        </p>
        {busy && (
          <p role="status">
            主题插画已完成 {completed}/3，可能需要几分钟…{" "}
            <button
              type="button"
              className="secondary"
              onClick={() => void client.cancel(requestRef.current)}
            >
              取消
            </button>
          </p>
        )}
        {message && <p role="status">{message}</p>}
        <div className={doc.cardList}>
          {proposals.map((proposal) => {
            const item = saved[proposal.name];
            return (
              <article className={doc.card} key={proposal.name}>
                <h3>{proposal.name}</h3>
                <p>{proposal.description}</p>
                <ScreenKeyboardPreview theme="light" skin="custom" customDesign={proposal.design} />
                <div className={doc.cardActions}>
                  <button
                    type="button"
                    className="primary"
                    onClick={() => {
                      onUse(proposal.design);
                      onClose();
                    }}
                  >
                    使用并继续编辑
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={Boolean(item)}
                    onClick={() => void save(proposal)}
                  >
                    {item ? "已保存" : "保存到我的皮肤"}
                  </button>
                  {communitySkins && (
                    <button
                      type="button"
                      className="secondary"
                      onClick={() =>
                        void save(proposal).then((value) => {
                          if (value) {
                            setPublishing(value);
                            setPublishDescription(proposal.description);
                          }
                        })
                      }
                    >
                      发布到社区
                    </button>
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
                onChange={(event) => setPublishing({ ...publishing, name: event.target.value })}
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
              <input
                type="checkbox"
                checked={publishAgreed}
                onChange={(event) => setPublishAgreed(event.target.checked)}
              />
              我拥有发布所用素材的权利，并同意其他用户免费下载使用
            </label>
            <p>发布后插画背景将公开，请勿包含私人或敏感资料。</p>
            <button
              type="button"
              className="primary"
              disabled={!publishAgreed || publishBusy}
              onClick={() => void publish()}
            >
              公开发布
            </button>
            <button
              type="button"
              className="secondary"
              disabled={publishBusy}
              onClick={() => setPublishing(null)}
            >
              取消
            </button>
          </div>
        )}
        <div className={community.dialogActions}>
          <button type="button" className="secondary" disabled={busy} onClick={onClose}>
            完成
          </button>
        </div>
      </section>
    </div>
  );
}

function libraryError(error: unknown): string {
  switch (errorCode(error)) {
    case "custom_skin_full":
      return "最多保存 12 套皮肤，请先删除不需要的设计。";
    case "custom_skin_invalid_name":
      return "请输入皮肤名称。";
    case "custom_skin_duplicate_name":
      return "已经有同名皮肤，请换一个名称。";
    case "custom_skin_not_found":
      return "这套皮肤已在其他窗口中变更，请重新打开图库。";
    case "custom_skin_format":
      return "皮肤图库无法读取，原文件已保留。";
    default:
      return "皮肤图库保存失败，请检查设备可用空间后重试。";
  }
}

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
}) {
  const [category, setCategory] = useState<Category>("背景");
  const [undo, setUndo] = useState<TouchKeyboardSkinDesign[]>([]);
  const [redo, setRedo] = useState<TouchKeyboardSkinDesign[]>([]);
  const [photoError, setPhotoError] = useState("");
  const [saved, setSaved] = useState<SavedTouchKeyboardSkin[]>([]);
  const [libraryBusy, setLibraryBusy] = useState(false);
  const [libraryNotice, setLibraryNotice] = useState("");
  const [nameEditor, setNameEditor] = useState<NameEditor | null>(null);
  const [skinName, setSkinName] = useState("");
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [aiGenerationOpen, setAiGenerationOpen] = useState(false);
  useEffect(() => {
    let current = true;
    if (!library)
      return () => {
        current = false;
      };
    setLibraryBusy(true);
    void library
      .load()
      .then((items) => {
        if (current) {
          setSaved(items);
          setLibraryNotice("");
        }
      })
      .catch((error) => {
        if (current) setLibraryNotice(libraryError(error));
      })
      .finally(() => {
        if (current) setLibraryBusy(false);
      });
    return () => {
      current = false;
    };
  }, [library]);
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
    if (!library) return false;
    setLibraryBusy(true);
    setLibraryNotice("");
    try {
      setSaved(await library.mutate(action));
      setLibraryNotice(success);
      return true;
    } catch (error) {
      setLibraryNotice(libraryError(error));
      return false;
    } finally {
      setLibraryBusy(false);
    }
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

  return (
    <div className={`${skin.editor} ${skin.editorFilled}`} aria-label="自定义皮肤编辑器">
      <div className={skin.editorHeading}>
        <div>
          <div className="section-title">
            自定义皮肤
            <small>Apple 同款当前设计字段；修改后使用页面底部“保存设置”写入共享配置</small>
          </div>
        </div>
        <div className={skin.editorHeadingActions}>
          {aiSkins && library && (
            <button
              type="button"
              className="primary"
              disabled={disabled || libraryBusy}
              onClick={() => setAiGenerationOpen(true)}
            >
              AI 皮肤抽卡
            </button>
          )}
          {library && (
            <button
              type="button"
              className="primary"
              disabled={disabled || libraryBusy || saved.length >= 12}
              onClick={() => {
                setSkinName(`我的设计 ${saved.length + 1}`);
                setNameEditor({ operation: "create" });
                setLibraryNotice("");
              }}
            >
              保存设计
            </button>
          )}
          <button type="button" className="secondary" onClick={onClose}>
            完成
          </button>
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
              onChange={(event) => setSkinName(truncateGraphemes(event.target.value, 32))}
            />
          </label>
          <div>
            <button
              type="button"
              className="primary"
              disabled={libraryBusy || !skinName.trim()}
              onClick={() => void submitName()}
            >
              {nameEditor.operation === "create" ? "确认保存" : "确认重命名"}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={libraryBusy}
              onClick={() => setNameEditor(null)}
            >
              取消
            </button>
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
            <button
              type="button"
              className={confirmation.operation === "delete" ? "danger" : "primary"}
              disabled={libraryBusy}
              onClick={() => void confirmLibraryMutation()}
            >
              {confirmation.operation === "update" ? "确认更新" : "确认删除"}
            </button>
            <button
              type="button"
              className="secondary"
              disabled={libraryBusy}
              onClick={() => setConfirmation(null)}
            >
              取消
            </button>
          </div>
        </div>
      )}
      {libraryNotice && (
        <p className={skin.libraryNotice} role="status">
          {libraryNotice}
        </p>
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
            <div className={skin.controlBlock}>
              <div className={skin.controlTitle}>背景预设</div>
              <div className={skin.backgroundGrid}>
                {touchKeyboardBackgroundPresets.map((preset) => (
                  <button
                    type="button"
                    aria-label={`背景预设 ${preset.title}`}
                    onClick={() =>
                      patch({
                        photo: undefined,
                        background: preset.start,
                        gradientEnd: preset.end,
                        gradientHorizontal: false,
                      })
                    }
                    key={preset.title}
                    style={{
                      background: `linear-gradient(135deg, ${skinColor(preset.start)}, ${skinColor(preset.end ?? preset.start)})`,
                    }}
                  >
                    <span>{preset.title}</span>
                  </button>
                ))}
              </div>
            </div>
            <div className={`${skin.controlBlock} ${skin.formGrid}`}>
              <label>
                背景起始色
                <input
                  className={skin.colorInput}
                  aria-label="背景起始色"
                  type="color"
                  value={skinColor(design.background)}
                  onChange={(event) => patch({ background: colorNumber(event.target.value) })}
                />
              </label>
              <label className={skin.checkLabel}>
                <input
                  aria-label="渐变背景"
                  type="checkbox"
                  checked={design.gradientEnd !== undefined}
                  onChange={(event) =>
                    patch({ gradientEnd: event.target.checked ? design.background : undefined })
                  }
                />
                渐变背景
              </label>
              {design.gradientEnd !== undefined && (
                <>
                  <label>
                    渐变结束色
                    <input
                      className={skin.colorInput}
                      aria-label="渐变结束色"
                      type="color"
                      value={skinColor(design.gradientEnd)}
                      onChange={(event) => patch({ gradientEnd: colorNumber(event.target.value) })}
                    />
                  </label>
                  <label className={skin.checkLabel}>
                    <input
                      aria-label="横向渐变"
                      type="checkbox"
                      checked={design.gradientHorizontal ?? false}
                      onChange={(event) => patch({ gradientHorizontal: event.target.checked })}
                    />
                    横向渐变
                  </label>
                </>
              )}
            </div>
            <div className={skin.controlBlock}>
              <div className={skin.controlTitle}>照片壁纸</div>
              <label className={`secondary ${skin.photoButton}`}>
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
                    void boundedPhoto(file)
                      .then((photo) => patch({ photo }))
                      .catch(() => setPhotoError("无法读取或压缩这张照片，请换一张再试。"));
                  }}
                />
              </label>
              {photoError && (
                <p role="alert" className={skin.warning}>
                  {photoError}
                </p>
              )}
              {design.photo && (
                <div className={skin.formGrid}>
                  <label>
                    照片位置 · {Math.round((design.photoPosition ?? 0.5) * 100)}%
                    <input
                      aria-label="照片位置"
                      type="range"
                      min="0"
                      max="1"
                      step=".01"
                      value={design.photoPosition ?? 0.5}
                      onChange={(event) => patch({ photoPosition: Number(event.target.value) })}
                    />
                  </label>
                  <label>
                    压暗照片 · {Math.round((design.photoShade ?? 0.25) * 100)}%
                    <input
                      aria-label="压暗照片"
                      type="range"
                      min="0"
                      max=".8"
                      step=".01"
                      value={design.photoShade ?? 0.25}
                      onChange={(event) => patch({ photoShade: Number(event.target.value) })}
                    />
                  </label>
                  <button
                    type="button"
                    className="danger-text"
                    onClick={() => patch({ photo: undefined })}
                  >
                    移除照片
                  </button>
                </div>
              )}
            </div>
            <div className={`${skin.controlBlock} ${skin.formGrid}`}>
              <label>
                背景纹理
                <select
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
                </select>
              </label>
              {design.pattern !== 0 && (
                <label>
                  纹理强度 · {Math.round((design.patternOpacity ?? 0.15) * 100)}%
                  <input
                    aria-label="纹理强度"
                    type="range"
                    min="0"
                    max=".5"
                    step=".01"
                    value={design.patternOpacity ?? 0.15}
                    onChange={(event) => patch({ patternOpacity: Number(event.target.value) })}
                  />
                </label>
              )}
            </div>
          </>
        )}

        {category === "按键" && (
          <>
            <div className={`${skin.controlBlock} ${skin.formGrid}`}>
              <label>
                键帽颜色
                <input
                  className={skin.colorInput}
                  aria-label="键帽颜色"
                  type="color"
                  value={skinColor(design.keyBackground)}
                  onChange={(event) => patch({ keyBackground: colorNumber(event.target.value) })}
                />
              </label>
              <label>
                功能键颜色
                <input
                  className={skin.colorInput}
                  aria-label="功能键颜色"
                  type="color"
                  value={skinColor(design.actionBackground)}
                  onChange={(event) => patch({ actionBackground: colorNumber(event.target.value) })}
                />
              </label>
            </div>
            <div className={`${skin.controlBlock} ${skin.formGrid}`}>
              <label>
                键帽造型
                <select
                  aria-label="键帽造型"
                  value={design.keyShape ?? "rounded"}
                  onChange={(event) => patch({ keyShape: event.target.value as TouchSkinKeyShape })}
                >
                  <option value="rounded">圆角</option>
                  <option value="capsule">胶囊</option>
                  <option value="ticket">票券</option>
                  <option value="pebble">卵石</option>
                </select>
              </label>
              <label>
                键帽材质
                <select
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
                </select>
              </label>
              <label>
                键帽不透明度 · {Math.round((design.keyOpacity ?? 1) * 100)}%
                <input
                  aria-label="键帽不透明度"
                  type="range"
                  min=".25"
                  max="1"
                  step=".01"
                  value={design.keyOpacity ?? 1}
                  onChange={(event) => patch({ keyOpacity: Number(event.target.value) })}
                />
              </label>
              <label>
                圆角 · {Math.round(design.cornerRadius)}
                <input
                  aria-label="自定义键帽圆角"
                  type="range"
                  min="0"
                  max="20"
                  step="1"
                  value={design.cornerRadius}
                  onChange={(event) => patch({ cornerRadius: Number(event.target.value) })}
                />
              </label>
              <label>
                边框 · {design.borderWidth.toFixed(1)}
                <input
                  aria-label="自定义键帽边框"
                  type="range"
                  min="0"
                  max="2"
                  step=".5"
                  value={design.borderWidth}
                  onChange={(event) => patch({ borderWidth: Number(event.target.value) })}
                />
              </label>
              <label>
                阴影 · {Math.round(design.shadow * 100)}%
                <input
                  aria-label="自定义键帽阴影"
                  type="range"
                  min="0"
                  max=".4"
                  step=".05"
                  value={design.shadow}
                  onChange={(event) => patch({ shadow: Number(event.target.value) })}
                />
              </label>
              <label>
                边框颜色
                <input
                  className={skin.colorInput}
                  aria-label="边框颜色"
                  type="color"
                  value={skinColor(design.customBorderColor ?? design.accent)}
                  onChange={(event) =>
                    patch({ customBorderColor: colorNumber(event.target.value) })
                  }
                />
              </label>
            </div>
          </>
        )}

        {category === "文本" && (
          <div className={`${skin.controlBlock} ${skin.formGrid}`}>
            <label className={skin.checkLabel}>
              <input
                aria-label="等宽字形"
                type="checkbox"
                checked={design.monospaced}
                onChange={(event) => patch({ monospaced: event.target.checked })}
              />
              等宽字形
            </label>
            <label>
              按键文字
              <input
                className={skin.colorInput}
                aria-label="按键文字"
                type="color"
                value={skinColor(design.keyForeground)}
                onChange={(event) => patch({ keyForeground: colorNumber(event.target.value) })}
              />
            </label>
            <label>
              提示与工具栏
              <input
                className={skin.colorInput}
                aria-label="提示与工具栏"
                type="color"
                value={skinColor(design.accent)}
                onChange={(event) => patch({ accent: colorNumber(event.target.value) })}
              />
            </label>
            {!hasReadableSkinText(design) && (
              <p className={skin.warning}>部分文字与背景对比度偏低，建议调整配色。</p>
            )}
            <button type="button" className="secondary" onClick={optimizeContrast}>
              优化文字对比度
            </button>
          </div>
        )}

        {category === "设计" && (
          <>
            <div className={skin.templateGrid}>
              {touchKeyboardSkinTemplates.map((template) => (
                <button
                  type="button"
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
            <button
              type="button"
              className="danger-text"
              onClick={() => apply(defaultTouchKeyboardSkinDesign)}
            >
              重置我的皮肤
            </button>
          </>
        )}
        {category === "我的" && library && (
          <div className={skin.controlBlock} aria-label="我的皮肤图库">
            <div className={skin.controlTitle}>我的皮肤 · {saved.length}/12</div>
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
                      setLibraryNotice("已载入设计；点击“使用皮肤”并保存页面设置后会应用到键盘。");
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
                    <button
                      type="button"
                      className="secondary"
                      disabled={libraryBusy}
                      aria-label={`用当前设计更新 ${item.name}`}
                      onClick={() => setConfirmation({ operation: "update", item })}
                    >
                      更新
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      disabled={libraryBusy}
                      aria-label={`重命名 ${item.name}`}
                      onClick={() => {
                        setSkinName(item.name);
                        setNameEditor({ operation: "rename", id: item.id });
                        setLibraryNotice("");
                      }}
                    >
                      重命名
                    </button>
                    <button
                      type="button"
                      className="danger-text"
                      disabled={libraryBusy}
                      aria-label={`删除 ${item.name}`}
                      onClick={() => setConfirmation({ operation: "delete", item })}
                    >
                      删除
                    </button>
                  </div>
                </article>
              ))}
            </div>
          </div>
        )}
      </div>

      <div className={skin.editorPreview}>
        <div className={skin.editorActions}>
          <button
            type="button"
            className="secondary"
            disabled={!undo.length || disabled}
            onClick={stepBack}
          >
            撤销设计
          </button>
          <button
            type="button"
            className="secondary"
            disabled={!redo.length || disabled}
            onClick={stepForward}
          >
            重做
          </button>
          <button type="button" className="primary" disabled={disabled || selected} onClick={onUse}>
            {selected ? "正在使用" : "使用皮肤"}
          </button>
        </div>
        <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={design} />
      </div>
    </div>
  );
}
