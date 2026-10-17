import { ActionButton } from "../core/action-button";
import { resourceKindTitle } from "./community-helpers";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityInputField } from "./community-input-field";
import { CommunitySelectField } from "./community-select-field";
import { CommunityTextareaField } from "./community-textarea-field";
import { CommunityPublicationMetadataFields } from "./community-publication-metadata-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import * as style from "./community-style";
import {
  COMMUNITY_DICTIONARY_SOURCE_NOTE,
  useCommunityResourcePublication,
  type CommunityResourcePublicationOptions,
} from "./use-community-resource-publication";

export interface CommunityResourceEditorProps extends CommunityResourcePublicationOptions {
  onClose: () => void;
}

export function CommunityResourceEditor({
  client,
  kind,
  existing,
  localDictionary,
  onClose,
  onPublished,
}: CommunityResourceEditorProps) {
  const {
    name,
    description,
    prompt,
    entries,
    entryKind,
    code,
    word,
    weight,
    agreed,
    error,
    busy,
    onNameChange,
    onDescriptionChange,
    onPromptChange,
    onCodeChange,
    onWordChange,
    onWeightChange,
    onAgreedChange,
    onEntryKindChange,
    addEntry,
    removeEntry,
    submit,
  } = useCommunityResourcePublication({ client, kind, existing, localDictionary, onPublished });
  return (
    <CommunityDialogFrame
      title={existing ? "更新作品" : `发布${resourceKindTitle(kind)}`}
      titleClassName={style.dialogTitle}
      ariaLabel={existing ? "更新社区作品" : `发布${resourceKindTitle(kind)}`}
      busy={busy}
      onClose={onClose}
      error={error}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <CommunityPublicationMetadataFields
        name={name}
        description={description}
        agreed={agreed}
        busy={busy}
        nameLabel="作品名称"
        nameAriaLabel="社区作品名称"
        descriptionLabel="作品说明"
        descriptionAriaLabel="社区作品说明"
        descriptionRows={3}
        showAgreement={!existing}
        agreementText="我拥有发布所用内容的权利，并同意其他用户查看和使用"
        onNameChange={onNameChange}
        onDescriptionChange={onDescriptionChange}
        onAgreedChange={onAgreedChange}
      />
      {kind === "reply" ? (
        <CommunityTextareaField
          label="回复提示词"
          ariaLabel="社区回复提示词"
          maxLength={2000}
          rows={8}
          value={prompt}
          disabled={busy}
          onChange={onPromptChange}
        />
      ) : (
        <>
          <div className={style.entryForm}>
            <CommunitySelectField
              label="类型"
              ariaLabel="社区词条类型"
              value={entryKind}
              disabled={busy}
              onChange={onEntryKindChange}
            >
              <option value="pinyin">拼音</option>
              <option value="wubi">五笔</option>
              <option value="quick">快捷短语</option>
              <option value="english">英文</option>
            </CommunitySelectField>
            <CommunityInputField
              label="编码"
              ariaLabel="社区词条编码"
              value={code}
              disabled={busy}
              onChange={onCodeChange}
            />
            <CommunityInputField
              label="词语"
              ariaLabel="社区词条文字"
              value={word}
              disabled={busy}
              onChange={onWordChange}
            />
            <CommunityInputField
              label="权重"
              ariaLabel="社区词条权重"
              type="number"
              value={weight}
              disabled={busy}
              onChange={onWeightChange}
            />
            <ActionButton
              action={addEntry}
              className="secondary"
              disabled={busy}
              label="添加词条"
            />
          </div>
          <div className={style.entryList} aria-label={`待发布词条 ${entries.length}/128`}>
            {entries.map((item, index) => (
              <div key={`${item.kind}-${item.code}-${item.word}-${index}`}>
                <span>
                  {item.word} · <code>{item.code}</code> · {item.weight}
                </span>
                <ActionButton
                  action={() => removeEntry(index)}
                  className="secondary"
                  disabled={busy}
                  label="移除"
                />
              </div>
            ))}
          </div>
        </>
      )}
      <CommunityPublicationWarning>
        发布内容会公开展示。请勿包含 API
        Key、私人聊天内容或其他个人资料；发布后可在“我的作品”中下架。
      </CommunityPublicationWarning>
      {kind === "dictionary" && (
        <CommunityPublicationWarning>
          {COMMUNITY_DICTIONARY_SOURCE_NOTE}
        </CommunityPublicationWarning>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        <button type="submit" className="primary" disabled={busy}>
          {busy ? "正在发布…" : existing ? "发布新版本" : "公开发布"}
        </button>
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
