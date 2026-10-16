import { handleCommunityPublishKeyDown } from "./community-publish-validation";
import { candidateSkinMegabytes, communityLicenseLine } from "./community-helpers";
import * as style from "./community-style";
import { CommunityMetrics } from "./community-metrics";
import { StatusMessage } from "../core/status-message";
import { CommunityConfirmationActions } from "./community-confirmation-actions";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunityNotice } from "./community-notice";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunitySkinCategorySelect } from "./community-skin-category";
import { CommunitySelectField } from "./community-select-field";
import { ActionButton } from "../core/action-button";
import {
  useCandidateSkinPublication,
  candidateSkinAssetLicenses as assetLicenses,
  type CandidateSkinPublicationOptions,
} from "./use-candidate-skin-publication";

/** 服务端的包体积上限与打包大小并列，方便作者查看剩余空间。 */
const packageLimit = "2 MB";
const publishWarning =
  "仅上传 skin.toml 与 PNG/JPEG 图片（需包含预览图）；单个文件不超过 1 MB、合计不超过 2 MB、每边不超过 2048 像素。服务器会重新编码图片并去除元数据。登录期间修改本地皮肤，会自动同步到这款作品。";

export interface CandidateSkinPublishDialogProps extends CandidateSkinPublicationOptions {
  onClose: () => void;
  /** 未登录导致发布失败时的登录入口；未提供时只显示错误。 */
  onLogin?: () => void;
}

/** 根节点保持 div，可嵌入主题设置表单，避免提交冒泡保存偏好。 */
export function CandidateSkinPublishDialog({
  client,
  localSkins,
  initialSkinId,
  openSkinDirectory,
  readImage,
  onClose,
  onPublished,
  onLogin,
}: CandidateSkinPublishDialogProps) {
  const {
    options,
    optionsLoading,
    skinId,
    pack,
    packError,
    packLoading,
    previewless,
    drawing,
    drawFailed,
    licenseless,
    licenseChoice,
    customLicense,
    licenseValid,
    writingLicense,
    licenseFailed,
    name,
    description,
    agreed,
    visibility,
    category,
    error,
    signInRequired,
    openFailed,
    busy,
    ready,
    setSkinId,
    setVisibility,
    setLicenseChoice,
    setCustomLicense,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    changeCategory,
    drawPreview,
    writeLicense,
    submit,
    openFolder,
  } = useCandidateSkinPublication({
    client,
    localSkins,
    initialSkinId,
    openSkinDirectory,
    readImage,
    onPublished,
  });

  return (
    <CommunityDialogFrame
      title="发布候选窗口皮肤"
      titleClassName={style.dialogTitle}
      ariaLabel="发布候选窗口皮肤"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onKeyDown={(event) => handleCommunityPublishKeyDown(event, () => void submit())}
    >
      {optionsLoading && <StatusMessage role="status">正在读取本地皮肤…</StatusMessage>}
      {!optionsLoading && options.length === 0 && (
        <CommunityNotice>
          还没有可发布的外部皮肤，请先把皮肤文件夹放进皮肤目录，再在「主题」的外部皮肤中刷新。
        </CommunityNotice>
      )}
      {options.length > 0 && (
        <CommunitySelectField
          label="发布皮肤"
          ariaLabel="发布皮肤"
          value={skinId}
          disabled={busy}
          onChange={setSkinId}
        >
          {options.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name === item.id ? item.id : `${item.name}（${item.id}）`}
            </option>
          ))}
        </CommunitySelectField>
      )}
      <fieldset className={style.field} disabled={busy}>
        <legend>谁可以看到</legend>
        <label>
          <input
            type="radio"
            name="candidate-skin-visibility"
            checked={visibility === "public"}
            onChange={() => setVisibility("public")}
          />{" "}
          公开（所有人可下载）
        </label>
        <label>
          <input
            type="radio"
            name="candidate-skin-visibility"
            checked={visibility === "private"}
            onChange={() => setVisibility("private")}
          />{" "}
          仅自己可见
        </label>
      </fieldset>
      {packLoading && <StatusMessage role="status">正在检查皮肤包…</StatusMessage>}
      {packError && previewless && readImage && (
        <div className={style.confirmation} role="alert">
          <p>
            这款皮肤还没有预览图，社区要用它展示皮肤。可以按皮肤自己的配色和图片生成一张，保存到皮肤文件夹后继续发布。
          </p>
          {drawFailed && (
            <p>生成预览图失败，请重试，或自己在 skin.toml 中用 preview 指定一张图片。</p>
          )}
          <CommunityConfirmationActions>
            {openSkinDirectory && (
              <ActionButton
                action={() => void openFolder()}
                className="secondary"
                disabled={drawing}
                label="打开目录"
              />
            )}
            <ActionButton
              action={() => void drawPreview()}
              className="primary"
              disabled={drawing}
              label={drawing ? "正在生成…" : "生成预览图"}
            />
          </CommunityConfirmationActions>
          {openFailed && <p>无法打开皮肤目录，请重试。</p>}
        </div>
      )}
      {packError && licenseless && (
        <fieldset className={style.field} disabled={writingLicense}>
          <legend>素材授权</legend>
          <CommunityMetrics>
            公开发布需要注明别人可以怎样使用皮肤里的图片，选择后会写入 skin.toml。
          </CommunityMetrics>
          {assetLicenses.map((item) => (
            <label key={item.value}>
              <input
                type="radio"
                name="candidate-skin-asset-license"
                checked={licenseChoice === item.value}
                onChange={() => setLicenseChoice(item.value)}
              />{" "}
              {item.label}
            </label>
          ))}
          <label>
            <input
              type="radio"
              name="candidate-skin-asset-license"
              checked={licenseChoice === "other"}
              onChange={() => setLicenseChoice("other")}
            />{" "}
            其他
          </label>
          {licenseChoice === "other" && (
            <input
              className={style.fieldControl}
              aria-label="其他素材授权"
              placeholder="例如：仅限个人使用，不得转售"
              value={customLicense}
              onChange={(event) => setCustomLicense(event.target.value)}
            />
          )}
          {licenseChoice === "other" && customLicense.trim() !== "" && !licenseValid && (
            <CommunityMetrics role="alert">授权说明太长，请控制在 40 个汉字以内。</CommunityMetrics>
          )}
          {licenseFailed && (
            <CommunityMetrics role="alert">
              写入授权失败，请重试，或在 skin.toml 的 [license] 中自己填写 assets。
            </CommunityMetrics>
          )}
        </fieldset>
      )}
      {packError && !(previewless && readImage) && !licenseless && (
        <div className={style.confirmation} role="alert">
          <p>{packError}</p>
          {openSkinDirectory && (
            <CommunityConfirmationActions>
              <ActionButton
                action={() => void openFolder()}
                className="secondary"
                label="打开目录"
              />
            </CommunityConfirmationActions>
          )}
          {openFailed && <p>无法打开皮肤目录，请重试。</p>}
        </div>
      )}
      {pack && (
        <>
          <CommunityMetrics>
            {pack.fileCount} 个文件 · {candidateSkinMegabytes(pack.size)} / {packageLimit}
          </CommunityMetrics>
          <CommunityMetrics aria-label="皮肤授权">
            {communityLicenseLine(pack.license)}
          </CommunityMetrics>
          <CommunitySkinPublicationFields
            name={name}
            description={description}
            agreed={agreed}
            busy={busy}
            agreementText={
              visibility === "public"
                ? "我拥有发布所用素材的权利，并同意其他用户按上述授权免费下载使用"
                : "我拥有上传所用素材的权利"
            }
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
          <CommunityPublicationWarning>{publishWarning}</CommunityPublicationWarning>
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        {packError && licenseless && (
          <ActionButton
            action={() => void writeLicense()}
            className="primary"
            disabled={writingLicense || !licenseValid}
            label={writingLicense ? "正在写入…" : "使用此授权并继续"}
          />
        )}
        {!packError && (
          <ActionButton
            action={() => void submit()}
            className="primary"
            disabled={busy || !ready}
            label={busy ? "正在发布…" : visibility === "public" ? "公开发布" : "保存到我的皮肤库"}
          />
        )}
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
