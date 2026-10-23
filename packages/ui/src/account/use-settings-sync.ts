import { useEffect, useState } from "react";
import type {
  AccountPreferenceSchema,
  AccountPreferences,
  SettingsSyncClient,
} from "./account-model";
import { useAccountAction } from "./account-operation";

/** 管理设置同步的快照、确认选择和异步动作，旧客户端或账号结果不能覆盖当前状态。 */
export function useSettingsSync(client: SettingsSyncClient, userId: string) {
  const [schema, setSchema] = useState<AccountPreferenceSchema | null>(null);
  const [cloud, setCloud] = useState<AccountPreferences | null>(null);
  const [message, setMessage] = useState("");
  const [confirmation, setConfirmation] = useState<"upload" | "apply" | null>(null);
  const report = (text: string) => setMessage(text);
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    report,
    report,
    userId,
  );

  const read = async () => {
    const current = clientGeneration.current;
    const [nextSchema, nextCloud] = await Promise.all([client.schema(), client.load()]);
    if (!mounted.current || clientGeneration.current !== current) return;
    setSchema(nextSchema);
    setCloud(nextCloud);
  };

  const load = () => {
    if (busy || !mounted.current) return;
    void perform(read);
  };

  useEffect(() => {
    setSchema(null);
    setCloud(null);
    setMessage("");
    void perform(read, { allowBusy: true });
  }, [client, userId]);

  // 操作面板先关闭自己再报告所选项，所以由它交回打开时对应的那个操作。
  const runConfirmed = (operation = confirmation) => {
    if (!cloud || !schema || !operation || busy) return;
    setConfirmation(null);
    void perform(async () => {
      const current = clientGeneration.current;
      if (operation === "upload") {
        const next = await client.upload();
        if (!mounted.current || clientGeneration.current !== current) return;
        setCloud(next);
      } else {
        await client.apply(userId, cloud);
        if (!mounted.current || clientGeneration.current !== current) return;
      }
      report(
        operation === "upload"
          ? "本机设置已上传。"
          : (client.appliedMessage ?? "已应用云端设置。请重新打开键盘使部分设置生效。"),
      );
    });
  };

  const hasCloudSettings = Boolean(cloud && Object.keys(cloud.settings).length > 0);
  return {
    schema,
    cloud,
    message,
    confirmation,
    setConfirmation,
    busy,
    load,
    runConfirmed,
    hasCloudSettings,
  };
}
