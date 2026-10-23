import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import type { SettingsSyncClient } from "./account-model";
import * as account from "./account-style";
import { useSettingsSync } from "./use-settings-sync";

/** 上传本机设置、应用云端设置的卡片；鸿蒙手机因键盘扩展有独立沙箱，不展示它。 */
export function SettingsSyncCard({
  client,
  userId,
}: {
  client: SettingsSyncClient;
  userId: string;
}) {
  const {
    schema,
    cloud,
    message,
    confirmation,
    setConfirmation,
    busy,
    load,
    runConfirmed,
    hasCloudSettings,
  } = useSettingsSync(client, userId);

  return (
    <section className={`${account.section} ${account.stack}`}>
      <h2 className={account.heading}>设置同步</h2>
      <p className={account.note}>
        {client.description ??
          "同步输入方案、繁体输出、键盘声音与触感、词库学习开关和皮肤。凭据、联网授权及输入内容不会随设置上传。"}
      </p>
      {cloud && <p className={account.muted}>云端版本：{cloud.revision}</p>}
      <div className={account.actionRow}>
        <ActionButton
          action={() => void load()}
          ariaBusy={busy}
          className="secondary"
          disabled={busy}
          label="刷新云端设置"
        />
        <ActionButton
          action={() => setConfirmation("upload")}
          ariaBusy={busy}
          className={account.primary}
          disabled={busy || !cloud || !schema}
          label="上传本机设置"
        />
        <ActionButton
          action={() => setConfirmation("apply")}
          ariaBusy={busy}
          className="secondary"
          disabled={busy || !cloud || !schema || !hasCloudSettings}
          label="下载并应用云端设置"
        />
      </div>
      {busy && <StatusMessage role="status">正在处理…</StatusMessage>}
      {message && <StatusMessage role="status">{message}</StatusMessage>}
      {confirmation && (
        <div
          className={account.confirmation}
          role="alertdialog"
          aria-label={confirmation === "upload" ? "确认上传本机设置" : "确认应用云端设置"}
        >
          <p className={account.note}>
            {confirmation === "upload"
              ? "将更新云端对应设置，并保留其他平台专属设置。版本冲突时不会自动覆盖。"
              : "将替换本机对应设置，不会下载词库或开启数据上传。"}
          </p>
          <div className={account.actionRow}>
            <ActionButton
              action={() => void runConfirmed()}
              ariaBusy={busy}
              className={account.primary}
              disabled={busy}
              label={confirmation === "upload" ? "确认上传" : "确认应用"}
            />
            <ActionButton
              action={() => setConfirmation(null)}
              ariaBusy={busy}
              className="secondary"
              disabled={busy}
              label="取消"
            />
          </div>
        </div>
      )}
    </section>
  );
}
