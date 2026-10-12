import { useEffect, useState } from "react";
import { GroupList } from "../core/platform-controls";
import { ActionButton } from "./action-button";
import {
  cloudClipboardRelayTokenIssue,
  cloudClipboardRelayUrlIssue,
  type CloudClipboardRelayPreferences,
} from "./cloud-clipboard-relay";
import { EndpointSettingField } from "./endpoint-setting-field";
import { SecretSettingField } from "./secret-setting-field";
import { SegmentedRow } from "./segmented-row";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";

type SyncMode = "account" | "relay";

export interface CloudClipboardRelaySectionProps {
  /** 已保存的中继配置；没有配置时为 `undefined`。 */
  relay: CloudClipboardRelayPreferences | undefined;
  disabled?: boolean;
  /** 写入一份校验过的中继配置，或者传 `undefined` 改回水杉账号。 */
  onChange: (relay: CloudClipboardRelayPreferences | undefined) => void;
}

/** 「剪贴板」页的「云剪贴板同步方式」：水杉账号，或者用户自建的中继（`docs/cloud-clipboard-relay.md`）。地址和令牌先在本组里填好、校验通过后点「保存」才写进设置，不会把填到一半的地址交给自动保存。 */
export function CloudClipboardRelaySection({
  relay,
  disabled = false,
  onChange,
}: CloudClipboardRelaySectionProps) {
  const savedUrl = relay?.url ?? "";
  const savedToken = relay?.token ?? "";
  const configured = savedUrl !== "";
  const [mode, setMode] = useState<SyncMode>(configured ? "relay" : "account");
  const [url, setUrl] = useState(savedUrl);
  const [token, setToken] = useState(savedToken);

  // 另一个窗口或恢复默认改了已保存的配置时，跟着它走。
  useEffect(() => {
    setMode(savedUrl ? "relay" : "account");
    setUrl(savedUrl);
    setToken(savedToken);
  }, [savedUrl, savedToken]);

  const trimmedUrl = url.trim();
  const trimmedToken = token.trim();
  const urlIssue = cloudClipboardRelayUrlIssue(trimmedUrl);
  const tokenIssue = cloudClipboardRelayTokenIssue(trimmedToken);
  const changed = trimmedUrl !== savedUrl || trimmedToken !== savedToken;

  return (
    <GroupList title="云剪贴板同步方式">
      <SegmentedRow
        title="同步方式"
        description="自建中继不需要登录水杉账号：内容仍在本机加密，中继只保存密文，新设备照样要在已有设备上核对后点「允许」。"
        aria-label="云剪贴板同步方式"
        options={[
          { value: "account", label: "水杉账号" },
          { value: "relay", label: "自建中继" },
        ]}
        value={mode}
        disabled={disabled}
        onChange={setMode}
      />
      {mode === "relay" && (
        <>
          <EndpointSettingField
            label="中继地址"
            inputLabel="中继地址"
            description="例如 https://relay.example.com/msime。必须是 HTTPS，证书要由公网信任的机构签发（自签证书和私有 CA 连不上）；HTTP 只用于本机测试（127.0.0.1）。"
            placeholder="https://relay.example.com/msime"
            value={url}
            disabled={disabled}
            onChange={setUrl}
          />
          {url !== "" && urlIssue && (
            <SettingsGroupNote role="status">{urlIssue}</SettingsGroupNote>
          )}
          <SecretSettingField
            label="中继令牌"
            description="由中继管理员生成，每个令牌对应中继上一份独立的云剪贴板。令牌只存在本机的设置里，不会随设置文件导出或账号同步。"
            value={token}
            disabled={disabled}
            onChange={setToken}
          />
          {token !== "" && tokenIssue && (
            <SettingsGroupNote role="status">{tokenIssue}</SettingsGroupNote>
          )}
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton
                label="保存中继设置"
                disabled={disabled || !changed || Boolean(urlIssue || tokenIssue)}
                action={() => onChange({ url: trimmedUrl, token: trimmedToken })}
              />
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
      {mode === "account" && configured && (
        <SettingsManagerBlock>
          <SettingsGroupNote>
            云剪贴板现在经自建中继 {savedUrl}{" "}
            同步。改回水杉账号会清除本机保存的中继地址和令牌，中继上的内容不受影响。
          </SettingsGroupNote>
          <SettingsManagerActions>
            <ActionButton
              label="改回水杉账号"
              disabled={disabled}
              action={() => onChange(undefined)}
            />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      )}
    </GroupList>
  );
}
