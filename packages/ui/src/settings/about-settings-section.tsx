import type { ValidatedUpdate } from "./update-manifest";
import { AboutHeroSection } from "./about-hero-section";
import { DataDirectorySection, type DataDirectoryInfo } from "./data-directory-section";
import { DiagnosticLogsSection, type DiagnosticLogPreferences } from "./diagnostic-logs-section";
import { HelpFeedbackSection } from "./help-feedback-section";
import { LicenseUninstallSection } from "./license-uninstall-section";
import { TelemetrySection } from "./telemetry-section";
import { describeInstallerTrust } from "./update-manifest";
import * as doc from "./document-style";

type InstallerTrust = ReturnType<typeof describeInstallerTrust>;

export interface AboutSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  logo: string;
  description: string;
  currentAppVersion: string;
  updateStatus: string;
  updateBusy: boolean;
  availableUpdate: ValidatedUpdate | null;
  installerTrust: InstallerTrust | null;
  licenseUrl: string;
  privacyUrl: string;
  macos: boolean;
  linux: boolean;
  windows: boolean;
  mobile: boolean;
  dataDirectoryVisible: boolean;
  dataDirectory?: DataDirectoryInfo;
  dataDirectoryBusy: boolean;
  dataDirectoryResult: string;
  onCheckForUpdate: () => void;
  onOpenExternalUrl: (url: string) => void;
  onChooseDataDirectory: () => void;
  openThirdPartyLicenses?: () => Promise<void>;
  uninstallInputSource?: (removeUserData: boolean) => Promise<void>;
  removeUserData: boolean;
  uninstallBusy: boolean;
  uninstallConfirmation: boolean;
  uninstallResult: "success" | "error" | null;
  onRemoveUserDataChange: (value: boolean) => void;
  onRequestUninstall: () => void;
  onConfirmUninstall: () => void;
  onCancelUninstall: () => void;
  diagnosticVisible: boolean;
  diagnosticLog: DiagnosticLogPreferences;
  openDiagnosticLogDirectory?: () => Promise<void>;
  onDiagnosticLogChange: (patch: Partial<DiagnosticLogPreferences>) => void;
  onDiagnosticLogError: (message: string) => void;
  telemetryEnabled?: boolean;
  onTelemetryChange: (value: boolean) => void;
  onHelp: () => void;
  onFeedback: () => void;
}

/** Composes the about, maintenance, and support controls shared by hosts. */
export function AboutSettingsSection({
  disabled,
  hidden,
  logo,
  description,
  currentAppVersion,
  updateStatus,
  updateBusy,
  availableUpdate,
  installerTrust,
  licenseUrl,
  privacyUrl,
  macos,
  linux,
  windows,
  mobile,
  dataDirectoryVisible,
  dataDirectory,
  dataDirectoryBusy,
  dataDirectoryResult,
  onCheckForUpdate,
  onOpenExternalUrl,
  onChooseDataDirectory,
  openThirdPartyLicenses,
  uninstallInputSource,
  removeUserData,
  uninstallBusy,
  uninstallConfirmation,
  uninstallResult,
  onRemoveUserDataChange,
  onRequestUninstall,
  onConfirmUninstall,
  onCancelUninstall,
  diagnosticVisible,
  diagnosticLog,
  openDiagnosticLogDirectory,
  onDiagnosticLogChange,
  onDiagnosticLogError,
  telemetryEnabled,
  onTelemetryChange,
  onHelp,
  onFeedback,
}: AboutSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="关于">
      <AboutHeroSection logo={logo} description={description} />
      <div className={`section ${doc.linkList}`}>
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
              onClick={() => onOpenExternalUrl(availableUpdate.releaseUrl)}
            >
              前往下载
            </button>
          </div>
        )}
        <button type="button" className={doc.linkRow} onClick={() => onOpenExternalUrl(licenseUrl)}>
          <span className={doc.linkTitle}>开源许可协议</span>
          <span aria-hidden="true">↗</span>
        </button>
        <button type="button" className={doc.linkRow} onClick={() => onOpenExternalUrl(privacyUrl)}>
          <span className={doc.linkTitle}>隐私政策</span>
          <span aria-hidden="true">↗</span>
        </button>
      </div>
      <DataDirectorySection
        visible={dataDirectoryVisible}
        linux={linux}
        dataDirectory={dataDirectory}
        busy={dataDirectoryBusy}
        result={dataDirectoryResult}
        onChoose={onChooseDataDirectory}
      />
      {macos && (
        <LicenseUninstallSection
          openThirdPartyLicenses={openThirdPartyLicenses}
          uninstallInputSource={uninstallInputSource}
          removeUserData={removeUserData}
          uninstallBusy={uninstallBusy}
          uninstallConfirmation={uninstallConfirmation}
          uninstallResult={uninstallResult}
          onRemoveUserDataChange={onRemoveUserDataChange}
          onRequestUninstall={onRequestUninstall}
          onConfirmUninstall={onConfirmUninstall}
          onCancelUninstall={onCancelUninstall}
        />
      )}
      <HelpFeedbackSection visible={mobile} onHelp={onHelp} onFeedback={onFeedback} />
      <DiagnosticLogsSection
        visible={diagnosticVisible}
        linux={linux}
        macos={macos}
        windows={windows}
        values={diagnosticLog}
        openDirectory={openDiagnosticLogDirectory}
        onChange={onDiagnosticLogChange}
        onError={onDiagnosticLogError}
      />
      {windows && <TelemetrySection value={telemetryEnabled} onChange={onTelemetryChange} />}
    </fieldset>
  );
}
