import { GroupList, Row, Switch } from "../core/platform-controls";

export interface TelemetrySectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Anonymous Windows Server usage reporting switch, the 隐私 group of the 关于 page. */
export function TelemetrySection({ value, onChange }: TelemetrySectionProps) {
  return (
    <GroupList title="隐私">
      <Row
        title="匿名使用统计"
        description="默认关闭。开启后，Server 每次启动向 https://api.msime.app/v1/telemetry/events 发送一条事件，只含随机事件 id、类型、平台名 windows 和版本号；Server 崩溃时再发一条，另带固定文本 std::terminate。不含输入内容、候选、剪贴板或账号信息。"
      >
        <Switch checked={value ?? false} onChange={onChange} />
      </Row>
    </GroupList>
  );
}
