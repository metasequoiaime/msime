import { GroupList } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsGroupNote } from "./settings-group-note";
import {
  networkProxyServerIssue,
  updateNetworkProxy,
  type NetworkProxyPreferences,
} from "./network-proxy-preferences";

export interface NetworkProxySectionProps {
  value: NetworkProxyPreferences | undefined;
  disabled?: boolean;
  onChange: (value: NetworkProxyPreferences) => void;
}

/**
 * 「AI 与云联想」等页共用的网络代理设置，对应 Windows 正式版的 `network.proxy_mode` / `network.proxy_server`。
 *
 * 只有访问公网 `https` 的请求才可能走代理：本机、局域网目标和任何明文 `http` 目标一律直连，
 * 免得 Token 与输入内容在代理上以明文出现（见 `crates/client-core/src/network.rs`）。
 */
export function NetworkProxySection({
  value,
  disabled = false,
  onChange,
}: NetworkProxySectionProps) {
  const current = updateNetworkProxy(value, {});
  const custom = current.proxy_mode === "custom";
  const issue = networkProxyServerIssue(current.proxy_server);
  return (
    <GroupList title="网络">
      <SettingsGroupNote>
        设置页、账号与社区、更新检查、词库与模型下载、AI
        与语音的配置检测都按这里联网。本机与局域网地址 以及任何明文 http
        地址始终直连，不经过代理。输入法打字时发出的云候选与候选翻译由各平台宿主自己的
        网络栈发送，只跟随系统代理。
      </SettingsGroupNote>
      <SettingsManagerBlock>
        <SelectRow
          title="代理"
          description="跟随系统代理；或直接连接；或用下面的 HTTP 代理（公司、校园网和网络受限地区常用）。"
          value={current.proxy_mode}
          disabled={disabled}
          onChange={(event) => {
            const mode = event.target.value;
            if (mode === "system" || mode === "none" || mode === "custom") {
              onChange(updateNetworkProxy(current, { proxy_mode: mode }));
            }
          }}
        >
          <option value="system">跟随系统代理</option>
          <option value="none">直接连接</option>
          <option value="custom">使用自定义 HTTP 代理</option>
        </SelectRow>
        {custom && (
          <TextInputRow
            title="代理地址"
            description="形如 127.0.0.1:7890，也可以写 http://127.0.0.1:7890；留空时等同于「跟随系统代理」。"
            detail={issue || undefined}
            label="代理地址"
            placeholder="127.0.0.1:7890"
            value={current.proxy_server}
            disabled={disabled}
            onChange={(proxy_server) => onChange(updateNetworkProxy(current, { proxy_server }))}
          />
        )}
      </SettingsManagerBlock>
    </GroupList>
  );
}
