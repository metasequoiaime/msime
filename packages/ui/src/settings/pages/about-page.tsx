import * as doc from "../document-style";
import { logo } from "../settings-options";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow } from "../../core/platform-controls";
import { LicenseRows } from "../license-rows";
import { TelemetryRow } from "../telemetry-section";
import { AboutHeroSection } from "../about-hero-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { OtherPlatformDownloadRows } from "./download-page";

const privacyUrl = "https://msime.app/privacy/";
const androidPrivacyUrl = "https://msime.app/privacy/";
// Linux links to the data-flow document that ships with this code, as the Windows reference links its own PRIVACY.md; the Linux section of msime.app/privacy/ describes a host without an update check and with Secret Service credentials, and this one has the update check and keeps provider credentials in 0600 files.
const linuxPrivacyUrl = "https://github.com/metasequoiaime/msime/blob/develop/PRIVACY.md";

/** The 关于 page of the settings form: the brand header, 版本与更新 (with the rows of the former 其他平台下载 page) and 许可与隐私. Uninstalling lives on 维护与诊断. */
export function AboutSettingsPage() {
  const {
    client,
    linuxPlatform,
    macosPlatform,
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
    availableUpdate,
    currentAppVersion,
    openExternalUrl,
    checkForUpdate,
    installerTrust,
    chooseDataDirectory,
    selectPage,
  } = useSettingsForm();
  const { onCheckForUpdate, onTelemetryChange } = createAboutSettingsActions({
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  return (
    <fieldset disabled={busy} hidden={page !== "about"} aria-label="关于">
      <div className={settings.groups}>
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
            <button
              type="button"
              className={`secondary ${doc.updateButton}`}
              disabled={updateBusy}
              onClick={onCheckForUpdate}
            >
              {updateBusy ? "正在检查…" : "检查更新"}
            </button>
          </div>
          {availableUpdate && (
            <div className={doc.updateResult}>
              <p>水杉 IME v{availableUpdate.version.display} 已发布。</p>
              {installerTrust?.warning && (
                <p className={doc.updateWarning}>{installerTrust.warning}</p>
              )}
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
              <button
                type="button"
                className="secondary"
                onClick={() => void openExternalUrl(availableUpdate.releaseUrl)}
              >
                前往下载
              </button>
            </div>
          )}
          <OtherPlatformDownloadRows />
        </GroupList>
        <GroupList title="许可与隐私">
          <LinkRow
            title="开源许可协议"
            external
            onClick={() => void openExternalUrl(platformLicenseUrl)}
          />
          {macosPlatform && <LicenseRows openThirdPartyLicenses={client.openThirdPartyLicenses} />}
          <LinkRow
            title="隐私政策"
            external
            onClick={() =>
              void openExternalUrl(
                linuxPlatform
                  ? linuxPrivacyUrl
                  : clientHostedPlatform
                    ? androidPrivacyUrl
                    : privacyUrl,
              )
            }
          />
          <TelemetryRow value={draft?.usage_reporting} onChange={onTelemetryChange} />
        </GroupList>
      </div>
    </fieldset>
  );
}
