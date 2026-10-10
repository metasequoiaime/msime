import * as doc from "../document-style";
import { logo } from "../settings-options";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, Row } from "../../core/platform-controls";
import { LicenseRows } from "../license-rows";
import { TelemetryRow, TelemetrySection } from "../telemetry-section";
import { AboutHeroSection, HarmonyAboutHero } from "../about-hero-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { OtherPlatformDownloadRows } from "./download-page";
import { ActionButton } from "../action-button";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { downloadMirrorPrefix } from "../app-resources";
import { mirrorDownloadUrl } from "../update-manifest";
import { SystemInfoSection } from "../system-info-section";
import { schemeTitle } from "../label-helpers";

const privacyUrl = "https://msime.app/privacy/";
const androidPrivacyUrl = "https://msime.app/privacy/";
/** 当前运行的版本已是最新时 `useUpdateCheck` 报告的内容。 */
const latestStatus = "已是最新版本";

/** 设置表单的「关于」页：品牌头部、「版本与更新」（含原「其他平台下载」页的几行）、「系统信息」（反馈问题时一键复制，#6644）和「许可与隐私」。卸载在「维护与诊断」。HarmonyOS（手机与 2in1）按新设计换成带版本行和检查更新胶囊的页首、「法律信息」组和单独的「隐私」组。 */
export function AboutSettingsPage() {
  const {
    client,
    linuxPlatform,
    macosPlatform,
    windowsPlatform,
    mobilePlatform,
    settingsPlatform,
    clientHostedPlatform,
    platformLicenseUrl,
    platformAboutDescription,
    draft,
    setDraft,
    busy,
    confirmUninstall,
    page,
    updateStatus,
    updateBusy,
    updateCheckSupported,
    availableUpdate,
    currentAppVersion,
    openExternalUrl,
    checkForUpdate,
    installerTrust,
    chooseDataDirectory,
    selectPage,
    host,
  } = useSettingsForm();
  const { onCheckForUpdate, onTelemetryChange } = createAboutSettingsActions({
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  const mirrorUrl = availableUpdate && mirrorDownloadUrl(availableUpdate, downloadMirrorPrefix);
  const updateResult = availableUpdate && (
    <div className={doc.updateResult}>
      <p>水杉 IME v{availableUpdate.version.display} 已发布。</p>
      {installerTrust?.warning && <p className={doc.updateWarning}>{installerTrust.warning}</p>}
      {installerTrust?.verify && (
        <>
          <p>
            下载后请核对 SHA256：<code>{installerTrust.verify.sha256}</code>
          </p>
          <p>
            核对命令：<code>{installerTrust.verify.command}</code>
          </p>
        </>
      )}
      {/* 国内连 GitHub 发布页常常只有几十 KB/s，镜像是同一个文件，上面的 SHA256 照样适用。 */}
      {mirrorUrl && (
        <ActionButton action={() => void openExternalUrl(mirrorUrl)} label="国内镜像下载" />
      )}
      <ActionButton
        action={() => void openExternalUrl(availableUpdate.releaseUrl)}
        label="前往下载"
      />
    </div>
  );
  const openPrivacy = () =>
    void openExternalUrl(clientHostedPlatform && !linuxPlatform ? androidPrivacyUrl : privacyUrl);
  if (settingsPlatform === "harmony" || settingsPlatform === "hm2") {
    // 只有结果为「已是最新」（"current"）时胶囊才把检查结果并入自己的标签；其他结果（有新版本、尚未发布、检查失败）都是用户需要读的一句话，所以仍作为状态行放在版权信息下方。
    const latest = !updateBusy && updateStatus === latestStatus;
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "about"} ariaLabel="关于">
        <GroupList>
          <HarmonyAboutHero
            version={currentAppVersion}
            platformLabel={settingsPlatform === "hm2" ? "HarmonyOS 2in1" : "HarmonyOS"}
            update={updateBusy ? "checking" : latest ? "latest" : "idle"}
            updateStatus={updateBusy || latest ? undefined : updateStatus}
            onCheckForUpdate={updateCheckSupported ? onCheckForUpdate : undefined}
          />
        </GroupList>
        {(updateResult || !mobilePlatform) && (
          <GroupList title="版本与更新">
            {updateResult}
            <OtherPlatformDownloadRows />
          </GroupList>
        )}
        <GroupList title="法律信息">
          <Row title="隐私政策">
            <ActionButton
              action={openPrivacy}
              className={doc.rowButton}
              ariaLabel="查看隐私政策"
              label="查看"
            />
          </Row>
          <Row title="开源许可">
            <ActionButton
              action={() => void openExternalUrl(platformLicenseUrl)}
              className={doc.rowButton}
              ariaLabel="查看开源许可"
              label="查看"
            />
          </Row>
        </GroupList>
        <TelemetrySection
          value={draft?.usage_reporting}
          onChange={onTelemetryChange}
          onOpenDetails={() => selectPage("usage-reporting")}
        />
      </SettingsPageFieldset>
    );
  }
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "about"} ariaLabel="关于">
      <GroupList>
        <AboutHeroSection logo={logo} description={platformAboutDescription} />
      </GroupList>
      <GroupList title="版本与更新">
        <div className={`${doc.linkRow} ${doc.versionRow}`}>
          <div>
            <div className={doc.linkTitle}>当前版本</div>
            <div className={doc.version}>v{currentAppVersion}</div>
            {updateStatus && (
              <p className={doc.updateStatus} role="status">
                {updateStatus}
              </p>
            )}
          </div>
          {updateCheckSupported && (
            <ActionButton
              action={onCheckForUpdate}
              className={`secondary ${doc.updateButton}`}
              disabled={updateBusy}
              label={updateBusy ? "正在检查…" : "检查更新"}
            />
          )}
        </div>
        {updateResult}
        <OtherPlatformDownloadRows />
      </GroupList>
      <SystemInfoSection
        appVersion={currentAppVersion}
        host={host}
        scheme={schemeTitle(draft.scheme)}
        copyText={client.copyText}
      />
      <GroupList title="许可与隐私">
        <LinkRow
          title="开源许可协议"
          external
          onClick={() => void openExternalUrl(platformLicenseUrl)}
        />
        {/* 第三方声明随 macOS 应用资源和 Windows 安装目录提供，两个宿主都能打开它。 */}
        {(macosPlatform || windowsPlatform) && (
          <LicenseRows openThirdPartyLicenses={client.openThirdPartyLicenses} />
        )}
        <LinkRow title="隐私政策" external onClick={openPrivacy} />
        <TelemetryRow
          value={draft?.usage_reporting}
          onChange={onTelemetryChange}
          onOpenDetails={() => selectPage("usage-reporting")}
        />
      </GroupList>
    </SettingsPageFieldset>
  );
}
