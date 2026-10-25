import { ControlledCheckbox } from "../core/controlled-checkbox";
import { StatusMessage } from "../core/status-message";
import { ActionButton } from "../core/action-button";
import { fullScreenColumn, fullScreenSurface } from "../core/full-screen-style";
import { useFullScreenScrollToTop } from "../core/use-full-screen-scroll-to-top";
import { ScreenKeyboardPreview } from "./screen-keyboard-preview";
import type { PreviewKey } from "./keyboard-layouts";
import type { TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";
import { useAiSkinGeneration, type UseAiSkinGenerationOptions } from "./use-ai-skin-generation";
import {
  communitySkinCategories,
  communitySkinCategoryLabels,
  type CommunitySkinCategory,
} from "../community/community-skin-category-model";
import * as doc from "../settings/document-style";
import * as community from "../community/community-style";
import { SettingsPageHeader } from "../settings/settings-page-header";

export interface AiSkinGenerationProps extends UseAiSkinGenerationOptions {
  fullScreen?: boolean;
  phonePreview?: boolean;
  /** 手机全屏视图使用手机预览；HarmonyOS 传入当前中文键面。 */
  touchRows?: PreviewKey[][];
  useLabel?: string;
  onUse: (design: TouchKeyboardSkinDesign) => void;
  onClose: () => void;
}

/**
 * AI 皮肤抽卡：每次抽三款皮肤，每款都可以使用、保存到「我的皮肤」或发布。编辑器以对话框形式在页面上打开它。`fullScreen` 则把它呈现为独立视图，标题前带返回按钮，手机皮肤网格里的 AI 磁贴就是这样打开它的。`useLabel` 是把方案交给 `onUse` 的那个按钮的名称。
 */
export function AiSkinGeneration({
  client,
  library,
  communitySkins,
  fullScreen = false,
  phonePreview = false,
  touchRows,
  useLabel = "使用并继续编辑",
  onUse,
  onClose,
}: AiSkinGenerationProps) {
  const {
    proposals,
    busy,
    completed,
    message,
    saved,
    publishing,
    publishDescription,
    publishAgreed,
    publishCategory,
    publishBusy,
    generate,
    cancelGeneration,
    save,
    beginPublish,
    publish,
    changePublishName,
    setPublishDescription,
    setPublishAgreed,
    setPublishCategory,
    closePublish,
  } = useAiSkinGeneration({ client, library, communitySkins });
  // 全屏视图叠在皮肤页上，共用外壳的滚动容器：不滚回顶部的话，它会停在皮肤页原来的滚动位置，标题和返回按钮都在屏幕外。
  const surface = useFullScreenScrollToTop<HTMLElement>(fullScreen);
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
            const item = saved.get(proposal);
            return (
              <article className={doc.card} key={proposal.name}>
                <h3>{proposal.name}</h3>
                <p>{proposal.description}</p>
                <ScreenKeyboardPreview
                  theme="light"
                  skin="custom"
                  customDesign={proposal.design}
                  phone={fullScreen || phonePreview}
                  touchRows={touchRows}
                />
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
                      action={() => beginPublish(proposal)}
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
                onChange={(event) => changePublishName(event.target.value)}
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
              action={closePublish}
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
