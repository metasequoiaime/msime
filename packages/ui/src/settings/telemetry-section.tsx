import { SettingToggle } from "./setting-toggle";

export interface TelemetrySectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Anonymous Windows Server usage reporting switch. */
export function TelemetrySection({ value, onChange }: TelemetrySectionProps) {
  return (
    <SettingToggle
      label="匿名使用统计"
      description={
        <>
          默认关闭。开启后，Server 每次启动向 https://api.msime.app/v1/telemetry/events 发送一条
          事件，只含随机事件 id、类型、平台名 windows 和版本号；Server 崩溃时再发一条，另带固定文本
          std::terminate。不含输入内容、候选、剪贴板或账号信息。
        </>
      }
      ariaLabel="匿名使用统计"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
