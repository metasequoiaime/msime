import { ActionButton } from "../core/action-button";
import { StatusMessage } from "../core/status-message";
import type { AppIconClient } from "./account-model";
import { AccountStatusMessages } from "./account-status-messages";
import * as account from "./account-style";
import { useAppIconSettings } from "./use-app-icon-settings";

const appIconOptions = [
  { id: "classic", title: "原版", detail: "经典黑白，简洁如初", color: "#252525" },
  { id: "forest", title: "杉林", detail: "杉叶青绿，沉静自然", color: "#2f6b4f" },
  { id: "sky", title: "晴空", detail: "清透蓝调，轻盈明亮", color: "#4e8fc8" },
  { id: "dusk", title: "暮紫", detail: "晚霞淡紫，温柔入夜", color: "#71618f" },
  { id: "vermilion", title: "朱砂", detail: "朱红印记，纸上东方", color: "#b9473f" },
] as const;

export function AppIconSettingsCard({
  client,
  platform,
}: {
  client: AppIconClient;
  platform?: "android" | "ios";
}) {
  const { info, pending, error, choose } = useAppIconSettings(client);

  return (
    <section className={`${account.section} ${account.iconSettings}`}>
      <div>
        <h2 className={account.heading}>App 图标</h2>
        <p className={account.note}>
          给主屏幕上的水杉换个颜色。
          {platform === "android"
            ? "Android 会使用系统启动器的图标别名保存选择。"
            : platform === "ios"
              ? "iOS 会使用系统备用图标接口保存选择。"
              : "系统会使用平台提供的图标切换能力保存选择。"}
        </p>
      </div>
      {info === null && !error && <StatusMessage role="status">正在读取图标状态…</StatusMessage>}
      <AccountStatusMessages error={error} />
      {info && !info.supported && <p className={account.muted}>当前设备暂不支持更换 App 图标。</p>}
      {info && (
        <div className={account.iconGrid}>
          {appIconOptions.map((option) => {
            const selected = info.selected === option.id;
            const changing = pending === option.id;
            return (
              <ActionButton
                action={() => void choose(option.id)}
                ariaLabel={`${option.title}，${option.detail}`}
                ariaPressed={selected}
                className={account.iconCard(selected)}
                key={option.id}
                disabled={!info.supported || pending !== null}
                label={
                  <>
                    <span
                      className={account.iconPreview}
                      style={{ backgroundColor: option.color }}
                      aria-hidden="true"
                    >
                      杉
                    </span>
                    <span className={account.iconCopy}>
                      <strong>{option.title}</strong>
                      <small>{option.detail}</small>
                    </span>
                    <span className={account.iconState(selected)}>
                      {changing ? "更换中" : selected ? "使用中" : "使用此图标"}
                    </span>
                  </>
                }
              />
            );
          })}
        </div>
      )}
      <p className={account.muted}>更换后，系统启动器可能需要片刻刷新。随时可以切回原版。</p>
    </section>
  );
}
