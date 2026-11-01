import { GroupList } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import { EndpointSettingRow } from "./endpoint-setting-row";
import { SecretSettingRow } from "./secret-setting-row";
import { SegmentedRow } from "./segmented-row";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import {
  useCloudClipboardRelay,
  type UseCloudClipboardRelayOptions,
} from "./use-cloud-clipboard-relay";

export interface CloudClipboardRelaySectionProps extends UseCloudClipboardRelayOptions {}

/** WebDAV 选项下面那段说明：数据怎么存、什么时候同步、坚果云的配额、凭据和回收站。 */
export const CLOUD_CLIPBOARD_WEBDAV_NOTE =
  "内容在本机加密，网盘只保存密文；打开面板或复制时同步，没有实时推送；坚果云免费账号每 30 分钟 600 次请求与其他应用共用；建议使用专用账号或专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。";

/** 「剪贴板」页的「云剪贴板同步方式」：水杉账号、用户自建的中继（`docs/cloud-clipboard-relay.md`），或者宿主支持时的 WebDAV 网盘（`docs/cloud-clipboard-webdav.md`）。地址和凭据先在本组里填好、校验通过后点「保存」才写进设置，不会把填到一半的地址交给自动保存；WebDAV 保存前先「测试连接」，不通过不保存。 */
export function CloudClipboardRelaySection({
  relay,
  webdav,
  probeWebDav,
  disabled = false,
  onChange,
  onWebDavChange,
}: CloudClipboardRelaySectionProps) {
  const {
    mode,
    url,
    token,
    savedUrl,
    hasSavedToken,
    configured,
    urlIssue,
    tokenIssue,
    canSave,
    canSwitchToAccount,
    setMode,
    editUrl,
    editToken,
    save,
    switchToAccount,
    webdavAvailable,
    webdavConfigured,
    savedWebDavUrl,
    webdavUrl,
    webdavUsername,
    webdavPassword,
    hasSavedWebDavPassword,
    webdavUrlIssue,
    webdavUsernameIssue,
    webdavPasswordIssue,
    webdavProbe,
    canProbeWebDav,
    canSaveWebDav,
    editWebDavUrl,
    editWebDavUsername,
    editWebDavPassword,
    probeWebDav: runWebDavProbe,
    saveWebDav,
  } = useCloudClipboardRelay({ relay, webdav, probeWebDav, disabled, onChange, onWebDavChange });

  return (
    <GroupList title="云剪贴板同步方式">
      <SegmentedRow
        title="同步方式"
        description={
          webdavAvailable
            ? "自建中继和 WebDAV 都不需要登录水杉账号：内容仍在本机加密，中继或网盘只保存密文，新设备照样要在已有设备上核对后点「允许」。"
            : "自建中继不需要登录水杉账号：内容仍在本机加密，中继只保存密文，新设备照样要在已有设备上核对后点「允许」。"
        }
        aria-label="云剪贴板同步方式"
        options={[
          { value: "account", label: "水杉账号" },
          { value: "relay", label: "自建中继" },
          ...(webdavAvailable ? [{ value: "webdav" as const, label: "WebDAV" }] : []),
        ]}
        value={mode}
        disabled={disabled}
        onChange={setMode}
      />
      {mode === "relay" && (
        <>
          <EndpointSettingRow
            title="中继地址"
            inputLabel="中继地址"
            description="例如 https://relay.example.com/msime。必须是 HTTPS，证书要由公网信任的机构签发（自签证书和私有 CA 连不上）；HTTP 只用于本机测试（127.0.0.1）。"
            placeholder="https://relay.example.com/msime"
            value={url}
            disabled={disabled}
            onChange={editUrl}
          />
          {url !== "" && urlIssue && (
            <SettingsGroupNote role="status">{urlIssue}</SettingsGroupNote>
          )}
          <SecretSettingRow
            title="中继令牌"
            label="中继令牌"
            description="由中继管理员生成，每个令牌对应中继上一份独立的云剪贴板。令牌只存在本机的设置里，不会随设置文件导出或账号同步。"
            value={token}
            placeholder={hasSavedToken ? "留空以保留已保存的令牌" : undefined}
            disabled={disabled}
            onChange={editToken}
          />
          {token !== "" && tokenIssue && (
            <SettingsGroupNote role="status">{tokenIssue}</SettingsGroupNote>
          )}
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton label="保存中继设置" disabled={!canSave} action={save} />
            </SettingsManagerActions>
          </SettingsManagerBlock>
          {configured && (
            <SettingsGroupNote role="status">
              云剪贴板现在经 {savedUrl}{" "}
              同步。换了地址或令牌后，本机要在中继上重新登记，并在其他设备上重新允许。
            </SettingsGroupNote>
          )}
        </>
      )}
      {mode === "webdav" && webdavAvailable && (
        <>
          <EndpointSettingRow
            title="服务器地址"
            inputLabel="服务器地址"
            description="坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-clipboard 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。必须是 HTTPS，证书要由公网信任的机构签发。"
            placeholder="https://dav.jianguoyun.com/dav/"
            value={webdavUrl}
            disabled={disabled}
            onChange={editWebDavUrl}
          />
          {webdavUrl !== "" && webdavUrlIssue && (
            <SettingsGroupNote role="status">{webdavUrlIssue}</SettingsGroupNote>
          )}
          <EndpointSettingRow
            title="账号"
            inputLabel="账号"
            description="网盘的登录账号，坚果云是登录邮箱。"
            value={webdavUsername}
            disabled={disabled}
            onChange={editWebDavUsername}
          />
          {webdavUsername !== "" && webdavUsernameIssue && (
            <SettingsGroupNote role="status">{webdavUsernameIssue}</SettingsGroupNote>
          )}
          <SecretSettingRow
            title="应用密码"
            label="应用密码"
            description="在网盘的安全设置里为第三方应用生成的密码，不是登录密码。只存在本机的设置里，不会随设置文件导出或账号同步。"
            value={webdavPassword}
            placeholder={hasSavedWebDavPassword ? "留空以保留已保存的应用密码" : undefined}
            disabled={disabled}
            onChange={editWebDavPassword}
          />
          {webdavPassword !== "" && webdavPasswordIssue && (
            <SettingsGroupNote role="status">{webdavPasswordIssue}</SettingsGroupNote>
          )}
          <SettingsGroupNote>{CLOUD_CLIPBOARD_WEBDAV_NOTE}</SettingsGroupNote>
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton label="测试连接" disabled={!canProbeWebDav} action={runWebDavProbe} />
              <ActionButton
                label="保存 WebDAV 设置"
                disabled={!canSaveWebDav}
                action={saveWebDav}
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
          {webdavProbe.kind === "probing" && (
            <SettingsGroupNote role="status">正在测试连接……</SettingsGroupNote>
          )}
          {webdavProbe.kind === "passed" && (
            <SettingsGroupNote role="status">连接正常，可以保存。</SettingsGroupNote>
          )}
          {webdavProbe.kind === "failed" && (
            <SettingsGroupNote role="status">{webdavProbe.message}</SettingsGroupNote>
          )}
          {webdavConfigured && (
            <SettingsGroupNote role="status">
              云剪贴板现在经 WebDAV 网盘 {savedWebDavUrl}{" "}
              同步。换了地址或账号后，本机要重新登记，并在其他设备上重新允许；换应用密码不用。
            </SettingsGroupNote>
          )}
        </>
      )}
      {mode === "account" && webdavConfigured && (
        <SettingsManagerBlock>
          <SettingsGroupNote>
            云剪贴板现在经 WebDAV 网盘 {savedWebDavUrl}{" "}
            同步。改回水杉账号会清除本机保存的地址、账号和应用密码，网盘上的内容不受影响。
          </SettingsGroupNote>
          <SettingsManagerActions>
            <ActionButton
              label="改回水杉账号"
              disabled={!canSwitchToAccount}
              action={switchToAccount}
            />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      )}
      {mode === "account" && configured && (
        <SettingsManagerBlock>
          <SettingsGroupNote>
            云剪贴板现在经自建中继 {savedUrl}{" "}
            同步。改回水杉账号会清除本机保存的中继地址和令牌，中继上的内容不受影响。
          </SettingsGroupNote>
          <SettingsManagerActions>
            <ActionButton
              label="改回水杉账号"
              disabled={!canSwitchToAccount}
              action={switchToAccount}
            />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      )}
    </GroupList>
  );
}
