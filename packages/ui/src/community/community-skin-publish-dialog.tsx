import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import { StatusMessage } from "../core/status-message";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityNotice } from "./community-notice";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunitySelectField } from "./community-select-field";
import { CommunitySkinCategorySelect } from "./community-skin-category";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import * as style from "./community-style";
import {
  useCommunitySkinPublication,
  type CommunitySkinPublicationOptions,
} from "./use-community-skin-publication";

export interface CommunitySkinPublishDialogProps extends CommunitySkinPublicationOptions {
  onClose: () => void;
  /** 手机发布表单使用手机键盘比例；HarmonyOS 还传入当前中文键面。 */
  phonePreview?: boolean;
  touchRows?: PreviewKey[][];
  /** 未登录导致发布失败时的登录入口；未提供时只显示错误。 */
  onLogin?: () => void;
}

export function CommunitySkinPublishDialog({
  client,
  library,
  onClose,
  phonePreview = false,
  touchRows,
  onPublished,
  onLogin,
}: CommunitySkinPublishDialogProps) {
  const {
    saved,
    selectedId,
    selected,
    name,
    description,
    agreed,
    category,
    busy,
    error,
    signInRequired,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    selectDesign,
    changeCategory,
    submit,
  } = useCommunitySkinPublication({ client, library, onPublished });

  return (
    <CommunityDialogFrame
      title="发布我的皮肤"
      ariaLabel="发布我的皮肤"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      {busy && saved.length === 0 && <StatusMessage role="status">正在读取我的皮肤…</StatusMessage>}
      {!busy && saved.length === 0 && (
        <CommunityNotice>还没有命名保存的皮肤，请先在“设计我的皮肤”中保存一款。</CommunityNotice>
      )}
      {saved.length > 0 && (
        <>
          <CommunitySelectField
            label="发布设计"
            ariaLabel="发布设计"
            value={selectedId}
            disabled={busy}
            onChange={selectDesign}
          >
            {saved.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </CommunitySelectField>
          {selected && (
            <div
              className={`${style.cardStage} ${phonePreview ? "mx-auto w-full max-w-[300px]" : "max-h-[220px]"}`}
            >
              <ScreenKeyboardPreview
                theme="light"
                skin="custom"
                customDesign={selected.design}
                phone={phonePreview}
                touchRows={touchRows}
              />
            </div>
          )}
          <CommunitySkinPublicationFields
            name={name}
            description={description}
            agreed={agreed}
            busy={busy}
            agreementText="我拥有发布所用素材的权利，并同意其他用户免费下载使用"
            onNameChange={onNameChange}
            onDescriptionChange={onDescriptionChange}
            onAgreedChange={onAgreedChange}
          />
          <CommunitySkinCategorySelect
            ariaLabel="发布分类"
            value={category}
            disabled={busy}
            onChange={changeCategory}
          />
          <CommunityPublicationWarning>
            发布后设计及照片壁纸将公开。请勿包含私人照片或敏感信息；发布成功后可在“我的作品”中下架。
          </CommunityPublicationWarning>
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        <button type="submit" className="primary" disabled={busy || !selected || !agreed}>
          {busy ? "正在发布…" : "公开发布"}
        </button>
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
