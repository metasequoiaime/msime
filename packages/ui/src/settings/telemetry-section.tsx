import { GroupList, LinkRow, Row } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";

export interface TelemetrySectionProps {
  /** `usage_reporting`; an absent value means on, the default. */
  value?: boolean;
  onChange: (value: boolean) => void;
  /** 打开「发送哪些内容」页；不给时不显示那一行（例如没有设置页导航的单独渲染）。 */
  onOpenDetails?: () => void;
}

/** 开关下的一句话。发送什么、包含什么的完整说明在「发送哪些内容」页（`UsageReportingDetails`）：整段写在开关下，手机上会撑成一大块文字。 */
export const usageReportingSummary = "不含输入内容、候选和剪贴板；关闭后不再发送。";

/** 匿名使用统计发送的上报地址，`crate::telemetry` 的 `EVENTS_PATH`。 */
export const usageReportingEndpoint = "https://api.msime.app/v1/telemetry/events";

/** 「匿名使用统计」开关这一行，后面是打开「发送哪些内容」页的一行。不带组；关于页把它放在「许可与隐私」组里隐私政策之后。 */
export function TelemetryRow({ value, onChange, onOpenDetails }: TelemetrySectionProps) {
  return (
    <>
      <SwitchRow
        title="匿名使用统计"
        description={usageReportingSummary}
        checked={value ?? true}
        onChange={onChange}
      />
      {onOpenDetails && <LinkRow title="发送哪些内容" onClick={onOpenDetails} />}
    </>
  );
}

/** 自成一个「隐私」组的匿名使用统计开关，留给自己拼页面的宿主。 */
export function TelemetrySection(props: TelemetrySectionProps) {
  return (
    <GroupList title="隐私">
      <TelemetryRow {...props} />
    </GroupList>
  );
}

/**
 * 「发送哪些内容」页的内容：匿名使用统计发送什么、每条包含什么、不包含什么，与 PRIVACY.md 的「匿名使用统计」一节说的是同一回事，按条拆成短行，手机上一屏读得完。上报的实际行为在 `crate::telemetry`。
 */
export function UsageReportingDetails() {
  return (
    <>
      <GroupList title="发送什么">
        <Row title="每天最多一条活跃记录" />
        <Row title="输入法进程结束或崩溃后一条会话记录" />
        <Row title="崩溃时另带错误摘要和调用栈" description="模块路径只保留文件名" />
      </GroupList>
      <GroupList title="每条包含">
        <Row title="随机事件 id、类型、平台和版本号" />
        <Row title="本机随机生成的安装 id" />
      </GroupList>
      <GroupList title="不包含">
        <Row title="输入内容、候选和剪贴板" />
        <Row title="账号、设备硬件和用户目录信息" />
      </GroupList>
      <GroupList title="其他">
        <Row title="默认开启" description="可随时在「关于」里关闭" />
        <Row title="发送失败时" description="在本机最多保留 64 条，稍后重试" />
        <Row title="关闭后" description="不再发送，并清空尚未发送的记录" />
        <Row title="发送地址" description={usageReportingEndpoint} />
      </GroupList>
    </>
  );
}
