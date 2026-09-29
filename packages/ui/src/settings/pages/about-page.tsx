import * as doc from "../document-style";
import { logo } from "../settings-options";
import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList } from "../../core/platform-controls";
import { LicenseUninstallSection } from "../license-uninstall-section";
import { TelemetrySection } from "../telemetry-section";

const privacyUrl = "https://msime.app/privacy/";
const androidPrivacyUrl = "https://msime.app/privacy/";
// Linux links to the data-flow document that ships with this code, as the Windows reference links its own PRIVACY.md; the Linux section of msime.app/privacy/ describes a host without an update check and with Secret Service credentials, and this one has the update check and keeps provider credentials in 0600 files.
const linuxPrivacyUrl = "https://github.com/metasequoiaime/msime/blob/develop/PRIVACY.md";

/** The 关于 page of the settings form. */
export function AboutSettingsPage() {
  const {
    client,
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    clientHostedPlatform,
    platformLicenseUrl,
    platformAboutDescription,
    draft,
    setDraft,
    busy,
    removeUserDataOnUninstall,
    setRemoveUserDataOnUninstall,
    uninstallConfirmation,
    uninstallBusy,
    uninstallResult,
    requestUninstall,
    confirmUninstall,
    cancelUninstall,
    page,
    updateStatus,
    updateBusy,
    availableUpdate,
    currentAppVersion,
    openExternalUrl,
    checkForUpdate,
    installerTrust,
  } = useSettingsForm();
  return (
    <fieldset disabled={busy} hidden={page !== "about"} aria-label="关于">
      <div className={settings.groups}>
        <GroupList>
          <div className={doc.hero}>
            <div className={doc.mark}>
              <img src={logo} alt="水杉 IME" />
            </div>
            <div>
              <div className={doc.eyebrow}>Metasequoia IME</div>
              <div className={doc.heroTitle}>水杉 IME</div>
              <p>{platformAboutDescription}</p>
            </div>
          </div>
        </GroupList>
        <GroupList title="版本与条款">
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
              onClick={() => void checkForUpdate()}
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
          <button
            type="button"
            className={doc.linkRow}
            onClick={() => void openExternalUrl(platformLicenseUrl)}
          >
            <span className={doc.linkTitle}>开源许可协议</span>
            <span aria-hidden="true">↗</span>
          </button>
          <button
            type="button"
            className={doc.linkRow}
            onClick={() =>
              void openExternalUrl(
                linuxPlatform
                  ? linuxPrivacyUrl
                  : clientHostedPlatform
                    ? androidPrivacyUrl
                    : privacyUrl,
              )
            }
          >
            <span className={doc.linkTitle}>隐私政策</span>
            <span aria-hidden="true">↗</span>
          </button>
        </GroupList>
        {macosPlatform && (
          <LicenseUninstallSection
            openThirdPartyLicenses={client.openThirdPartyLicenses}
            uninstallInputSource={client.uninstallInputSource}
            removeUserData={removeUserDataOnUninstall}
            uninstallBusy={uninstallBusy}
            uninstallConfirmation={uninstallConfirmation}
            uninstallResult={uninstallResult}
            onRemoveUserDataChange={setRemoveUserDataOnUninstall}
            onRequestUninstall={requestUninstall}
            onConfirmUninstall={() => void confirmUninstall()}
            onCancelUninstall={cancelUninstall}
          />
        )}
        {/* Only the Windows Server reads this switch; the other hosts report on their own terms, described in PRIVACY.md, so offering it there would be a switch that changes nothing. */}
        {windowsPlatform && (
          <TelemetrySection
            value={draft?.telemetry_enabled}
            onChange={(telemetry_enabled) => setDraft({ ...draft, telemetry_enabled })}
          />
        )}
      </div>
    </fieldset>
  );
}
