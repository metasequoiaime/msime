import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";

export interface TelemetrySectionProps {
  /** `usage_reporting`; an absent value means on, the default. */
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** What usage reporting sends, word for word as PRIVACY.md describes it; every host reads the same `usage_reporting` switch. */
export const usageReportingDescription =
  "默认开启，可随时关闭。开启时，输入法向 https://api.msime.app/v1/telemetry/events 发送匿名使用统计：每天最多一条活跃记录；每次输入法进程正常结束或崩溃后一条会话记录；崩溃时另带错误摘要和调用栈（模块路径只保留文件名）。每条只含随机事件 id、类型、平台、版本号和本机随机生成的安装 id，不含输入内容、候选、剪贴板、账号、设备硬件或用户目录信息。发送失败的记录在本机最多保留 64 条，稍后重试。关闭后不再发送，并清空尚未发送的记录。";

/** 「匿名使用统计」开关这一行，不带组；关于页把它放在「许可与隐私」组里隐私政策之后。 */
export function TelemetryRow({ value, onChange }: TelemetrySectionProps) {
  return (
    <SwitchRow
      title="匿名使用统计"
      description={usageReportingDescription}
      checked={value ?? true}
      onChange={onChange}
    />
  );
}

/** 自成一个「隐私」组的匿名使用统计开关，留给自己拼页面的宿主。 */
export function TelemetrySection({ value, onChange }: TelemetrySectionProps) {
  return (
    <GroupList title="隐私">
      <TelemetryRow value={value} onChange={onChange} />
    </GroupList>
  );
}
