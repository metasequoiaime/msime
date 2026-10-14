import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { ControlledCheckbox } from "../core/controlled-checkbox";
import { errorCode } from "../core/error-code";
import { StatusMessage } from "../core/status-message";
import { ActionButton } from "../core/action-button";
import { trackAsyncSubscription } from "../core/async-subscription";
import { fullScreenColumn, fullScreenSurface } from "../core/full-screen-style";
import { randomRequestId } from "../core/random-id";
import { aiSkinMessage, libraryError } from "./touch-keyboard-skin-errors";
import { createAiSkinPrompt } from "./touch-keyboard-skin-ai";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import { boundTouchSkinArtwork } from "./touch-skin-images";
import {
  updateSavedTouchKeyboardSkinName,
  type AiSkinClient,
  type AiSkinProposal,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";
import type { CommunitySkinClient } from "../community/community-skins";
import {
  communitySkinCategories,
  communitySkinCategoryLabels,
  type CommunitySkinCategory,
} from "../community/community-skin-category";
import * as doc from "../settings/document-style";
import * as community from "../community/community-style";
import { SettingsPageHeader } from "../settings/settings-page-header";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useMountedRef } from "../settings/use-mounted-ref";

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
    return trackAsyncSubscription(
      client.onProgress((progress) => {
        if (generation === progressGeneration.current && progress.requestId === requestRef.current)
          setCompleted(progress.completed);
      }),
      () => generation === progressGeneration.current,
    );
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
