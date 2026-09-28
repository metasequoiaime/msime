export interface TelemetrySectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Anonymous Windows Server usage reporting switch. */
export function TelemetrySection({ value, onChange }: TelemetrySectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          匿名使用统计
          <small>
            默认关闭。开启后，Server 每次启动向 https://api.msime.app/v1/telemetry/events
            发送一条事件，只含随机事件 id、类型、平台名 windows 和版本号；Server
            崩溃时再发一条，另带固定文本 std::terminate。不含输入内容、候选、剪贴板或账号信息。
          </small>
        </span>
        <input
          aria-label="匿名使用统计"
          className="toggle"
          type="checkbox"
          checked={value ?? false}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
