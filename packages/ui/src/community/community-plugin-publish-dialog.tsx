import { StatusMessage } from "../core/status-message";
import { ActionButton } from "../core/action-button";
import { candidateSkinMegabytes } from "./community-helpers";
import { handleCommunityPublishKeyDown } from "./community-publish-validation";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunitySelectField } from "./community-select-field";
import { CommunityPublicationMetadataFields } from "./community-publication-metadata-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunityNotice } from "./community-notice";
import { CommunityMetrics } from "./community-metrics";
import * as style from "./community-style";
import {
  useCommunityPluginPublication,
  type CommunityPluginPublicationOptions,
} from "./use-community-plugin-publication";

/** 服务端的归档上限与打包大小并列，方便作者查看剩余空间。 */
const archiveLimit = "8 MB";
const publishWarning =
  "上传的是插件目录中的全部文件（不含隐藏文件），压缩后不超过 8 MB；每个账号最多发布 20 个、合计 32 MB，每小时最多发布 10 次。其他用户下载后会按原样安装。";

export interface CommunityPluginPublishDialogProps extends CommunityPluginPublicationOptions {
  onClose: () => void;
  onLogin?: () => void;
}

/** 宿主按类型与标识预检和打包本机插件；失败重试沿用同一发布 ID。 */
export function CommunityPluginPublishDialog({
  client,
  localPlugins,
  onClose,
  onPublished,
  onLogin,
  scriptPlugins = false,
}: CommunityPluginPublishDialogProps) {
  const {
    options,
    optionsLoading,
    selection,
    chosenKind,
    pack,
    packError,
    packLoading,
    name,
    description,
    agreed,
    busy,
    error,
    signInRequired,
    ready,
    selectPlugin,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    submit,
  } = useCommunityPluginPublication({ client, localPlugins, onPublished, scriptPlugins });
  const script = chosenKind === "script";

  return (
    <CommunityDialogFrame
      title="发布插件"
      titleClassName={style.dialogTitle}
      ariaLabel="发布插件"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onKeyDown={(event) => handleCommunityPublishKeyDown(event, () => void submit())}
    >
      {optionsLoading && <StatusMessage role="status">正在读取本地插件…</StatusMessage>}
      {!optionsLoading && options.length === 0 && (
        <CommunityNotice>
          {scriptPlugins
            ? "还没有可发布的插件。内置插件不能发布，请先在「我的插件」中导入自己的音效包、音乐包、指令表、特效包、短语表、辅助码表、单词本、符号集或脚本插件。"
            : "还没有可发布的插件。内置插件不能发布，请先在「我的插件」中导入自己的音效包、音乐包、指令表、特效包、短语表、辅助码表、单词本或符号集。"}
        </CommunityNotice>
      )}
      {options.length > 0 && (
        <CommunitySelectField
          label="发布插件"
          ariaLabel="发布插件"
          value={selection}
          disabled={busy}
          onChange={selectPlugin}
        >
          {options.map((item) => (
            <option key={item.key} value={item.key}>
              {item.label} · {item.name === item.id ? item.id : `${item.name}（${item.id}）`}
            </option>
          ))}
        </CommunitySelectField>
      )}
      {packLoading && <StatusMessage role="status">正在检查插件…</StatusMessage>}
      {packError && (
        <div className={style.confirmation} role="alert">
          <p>{packError}</p>
        </div>
      )}
      {pack && (
        <>
          <CommunityMetrics>
            {[
              `v${pack.version}`,
              pack.license && `授权 ${pack.license}`,
              `${pack.fileCount} 个文件`,
              `${candidateSkinMegabytes(pack.size)} / ${archiveLimit}`,
            ]
              .filter(Boolean)
              .join(" · ")}
          </CommunityMetrics>
          <CommunityPublicationMetadataFields
            name={name}
            description={description}
            agreed={agreed}
            busy={busy}
            nameLabel="名称"
            nameAriaLabel="发布插件名称"
            descriptionLabel="说明"
            descriptionAriaLabel="发布插件说明"
            agreementText={
              script
                ? "我拥有插件中代码与文字的发布权利，并同意其他用户按包内授权免费下载使用"
                : "我拥有插件中音频与文字的发布权利，并同意其他用户按包内授权免费下载使用"
            }
            agreementClassName={style.agreementBox}
            onNameChange={onNameChange}
            onDescriptionChange={onDescriptionChange}
            onAgreedChange={onAgreedChange}
          />
          <CommunityPublicationWarning>{publishWarning}</CommunityPublicationWarning>
          {script && (
            <CommunityPublicationWarning>
              脚本插件要经过管理员审核，通过后才会出现在社区里；审核前只有你自己在「我的作品」里看得到。其他用户安装后也不会自动运行，要在脚本插件页逐项授权。
            </CommunityPublicationWarning>
          )}
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        {!packError && (
          <ActionButton
            action={() => submit()}
            className="primary"
            disabled={busy || !ready}
            label={busy ? "正在发布…" : script ? "提交审核" : "公开发布"}
          />
        )}
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
