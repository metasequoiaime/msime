import { SettingsGroupNote } from "./settings-group-note";
import { HostActionButton } from "../keyboard/HostActionButton";
import { GroupList, Row } from "../core/platform-controls";

export interface InputMethodServiceSectionProps {
  visible: boolean;
  macos: boolean;
  linux: boolean;
  restartInputMethod?: () => Promise<void>;
  installInputSource?: () => Promise<void>;
}

/** Restart and installation actions for desktop input method services. */
export function InputMethodServiceSection({
  visible,
  macos,
  linux,
  restartInputMethod,
  installInputSource,
}: InputMethodServiceSectionProps) {
  if (!visible) return null;

  return (
    <GroupList title="输入法服务">
      <SettingsGroupNote>
        {macos
          ? "重新注册并启用已安装的水杉输入源；当前输入法进程继续按系统生命周期运行。"
          : linux
            ? "重启 IBus 输入法服务；使用 Fcitx5 时重载水杉插件，关闭并重建所有输入会话，不影响其他输入法。"
            : "请求受监督的输入法服务重新启动。"}
      </SettingsGroupNote>
      <Row title={macos ? "重新注册当前输入源" : "立即重启输入法服务"}>
        <HostActionButton
          action={restartInputMethod}
          label={macos ? "重新注册" : "重启"}
          success={
            macos ? "已重新注册输入源。" : linux ? "已请求重启输入法服务。" : "已发送重启请求。"
          }
          error={
            macos
              ? "重新注册输入源失败，请确认输入法已经安装。"
              : "重启输入法服务失败，请稍后重试。"
          }
        />
      </Row>
      {macos && installInputSource && (
        <Row
          title="安装或更新水杉输入源"
          description="将当前应用随附的 IMK bundle 安装到本机输入法目录，然后注册到系统。"
        >
          <HostActionButton
            action={installInputSource}
            label="安装 / 更新"
            success="输入源已安装并注册。"
            error="输入源安装或注册失败，请重试。"
          />
        </Row>
      )}
    </GroupList>
  );
}
