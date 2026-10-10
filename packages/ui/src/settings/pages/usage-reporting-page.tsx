import { SettingsPageFieldset } from "../settings-page-fieldset";
import { useSettingsForm } from "../settings-form-context";
import { UsageReportingDetails } from "../telemetry-section";

/** 「关于」里「匿名使用统计」开关下面那一行打开的二级页面「发送哪些内容」。 */
export function UsageReportingSettingsPage() {
  const { busy, page } = useSettingsForm();
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "usage-reporting"}
      ariaLabel="发送哪些内容"
    >
      <UsageReportingDetails />
    </SettingsPageFieldset>
  );
}
