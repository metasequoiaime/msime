import { useEffect, useState } from "react";
import { Row, Switch } from "../core/platform-controls";
import { FluentIcon } from "../core/fluent-icons";
import { useToast } from "../core/toast";
import { useAsyncActionRunner } from "../core/use-async-action";
import type { StatisticsAuthenticationSettings } from "../settings/statistics-authentication-gate";

/** 「我的」页上的本机认证开关，保存失败保留原选择。 */
export function StatisticsAuthenticationRow({
  client,
}: {
  client: StatisticsAuthenticationSettings;
}) {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [error, setError] = useState("");
  const toast = useToast();
  const { busy, run } = useAsyncActionRunner(setError, undefined, client);
  useEffect(() => {
    let current = true;
    setEnabled(null);
    setError("");
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
  function change(value: boolean) {
    void run(
      async (isCurrent) => {
        await client.save(value);
        if (isCurrent()) setEnabled(value);
      },
      {
        formatError: () => "",
        onError: () => toast("没有保存，请重试"),
      },
    );
  }
  return (
    <Row
      title="统计页身份验证"
      description="开启后使用系统认证，5 分钟内再次进入不用重新验证；未配置锁屏凭据时直接进入"
      detail={error || undefined}
      icon={<FluentIcon name="shield_lock" size={18} />}
    >
      <Switch
        checked={enabled ?? false}
        disabled={enabled === null || busy}
        onChange={(value) => {
          void change(value);
        }}
      />
    </Row>
  );
}
