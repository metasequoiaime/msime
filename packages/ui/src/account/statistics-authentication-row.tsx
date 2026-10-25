import { useEffect, useState } from "react";
import { Row, Switch } from "../core/platform-controls";
import { FluentIcon } from "../core/fluent-icons";
import { useToast } from "../core/toast";
import type { StatisticsAuthenticationSettings } from "../settings/statistics-authentication-gate";

/** 「我的」页上的本机认证开关，保存失败保留原选择。 */
export function StatisticsAuthenticationRow({
  client,
}: {
  client: StatisticsAuthenticationSettings;
}) {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const toast = useToast();
  useEffect(() => {
    let current = true;
    setEnabled(null);
    void client.load().then(
      (value) => {
        if (current) setEnabled(value);
      },
      () => {
        if (current) setError("认证设置读取失败，请重新打开此页。");
      },
    );
    return () => {
      current = false;
    };
  }, [client]);
  async function change(value: boolean) {
    setBusy(true);
    try {
      await client.save(value);
      setEnabled(value);
    } catch {
      toast("没有保存，请重试");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Row
      title="统计页身份验证"
      description={error || "开启后使用系统认证，未配置锁屏凭据时直接进入"}
      icon={<FluentIcon name="shield_lock" size={18} />}
    >
      <Switch
        checked={enabled ?? true}
        disabled={enabled === null || busy}
        onChange={(value) => {
          void change(value);
        }}
      />
    </Row>
  );
}
