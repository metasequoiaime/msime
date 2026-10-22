import * as doc from "../document-style";
import { logo } from "../settings-options";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, Row } from "../../core/platform-controls";
import { MascotIllustration } from "../../core/mascot";
import { LicenseRows } from "../license-rows";
import { TelemetryRow, TelemetrySection } from "../telemetry-section";
import { AboutHeroSection, HarmonyAboutHero } from "../about-hero-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { OtherPlatformDownloadRows } from "./download-page";
import { FeedbackSections } from "./feedback-page";
import { ActionButton } from "../action-button";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { androidPrivacyUrl, downloadMirrorPrefix, privacyUrl } from "../app-resources";
import { mirrorDownloadUrl } from "../update-manifest";
import { SystemInfoSection } from "../system-info-section";
import { schemeTitle } from "../label-helpers";
import { aboutTabs } from "../about-tabs";
import { useSettingsTabs } from "../use-settings-tabs";
import { formatAppVersion } from "../use-app-release-date";

/** 当前运行的版本已是最新时 `useUpdateCheck` 报告的内容。 */
const latestStatus = "已是最新版本";

/** 桌面关于页按功能分成顶部标签，手机保留直列；鸿蒙沿用自己的品牌头部与法律信息布局。 */
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
    desktopDownloadUrl,
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
    appReleaseDate,
    openExternalUrl,
    checkForUpdate,
    installerTrust,
    chooseDataDirectory,
    selectPage,
    host,
    setError,
    aboutTab,
    setAboutTab,
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
      <ActionButton action={() => void openExternalUrl(desktopDownloadUrl)} label="前往下载" />
    </div>
  );
  const openPrivacy = () =>
    void openExternalUrl(clientHostedPlatform && !linuxPlatform ? androidPrivacyUrl : privacyUrl);
  const harmony = settingsPlatform === "harmony" || settingsPlatform === "hm2";
  const { tabList, panel } = useSettingsTabs(
    aboutTabs.filter((tab) => tab.id !== "system" || !harmony),
    "关于设置分类",
    !mobilePlatform,
    aboutTab,
    setAboutTab,
  );
  // 最新版本并入鸿蒙的检查按钮，其余检查结果仍单独显示。
  const latest = !updateBusy && updateStatus === latestStatus;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "about"} ariaLabel="关于">
      {tabList}
      {panel(
        "version",
        <>
          <GroupList>
            {harmony ? (
              <HarmonyAboutHero
                version={currentAppVersion}
                releaseDate={appReleaseDate}
                platformLabel={settingsPlatform === "hm2" ? "HarmonyOS 2in1" : "HarmonyOS"}
                update={updateBusy ? "checking" : latest ? "latest" : "idle"}
                updateStatus={updateBusy || latest ? undefined : updateStatus}
                onCheckForUpdate={updateCheckSupported ? onCheckForUpdate : undefined}
              />
            ) : (
              <AboutHeroSection logo={logo} description={platformAboutDescription} />
            )}
          </GroupList>
          {(!harmony || updateResult || !mobilePlatform) && (
            <GroupList title="版本与更新">
              {!harmony && (
                <div className={`${doc.linkRow} ${doc.versionRow}`}>
                  <div>
                    <div className={doc.linkTitle}>当前版本</div>
                    <div className={doc.version}>
                      {formatAppVersion(currentAppVersion, appReleaseDate)}
                    </div>
                    {updateStatus && (
                      <p className={doc.updateStatus} role="status">
                        {updateStatus}
                      </p>
                    )}
                    {latest && <MascotIllustration scene="updated" size={64} className="mt-2" />}
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
              )}
              {updateResult}
              <OtherPlatformDownloadRows />
            </GroupList>
          )}
        </>,
      )}
      {!mobilePlatform && panel("feedback", <FeedbackSections />)}
      {!harmony &&
        panel(
          "system",
          <SystemInfoSection
            appVersion={currentAppVersion}
            releaseDate={appReleaseDate}
            host={host}
            scheme={schemeTitle(draft.scheme)}
            copyText={client.copyText}
            setError={setError}
          />,
        )}
      {panel(
        "privacy",
        harmony ? (
          <>
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
          </>
        ) : (
          <GroupList title="许可与隐私">
            <LinkRow
              title="开源许可协议"
              external
              onClick={() => void openExternalUrl(platformLicenseUrl)}
            />
            {/* 第三方声明随 macOS 应用资源和 Windows 安装目录提供。 */}
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
        ),
      )}
    </SettingsPageFieldset>
  );
}
