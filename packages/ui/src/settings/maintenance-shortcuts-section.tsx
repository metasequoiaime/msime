import * as settings from "./settings-style";
import { GroupList, Row } from "../core/platform-controls";

export interface MaintenanceShortcutsSectionProps {
  visible: boolean;
  macos: boolean;
  linux: boolean;
  maintenanceChord: string;
}

const key = (chord: string) => <kbd className={settings.shortcutKey}>{chord}</kbd>;
const danger = (text: string) => <span className={settings.shortcutRowDanger}>{text}</span>;

/** Maintenance shortcut guidance shared by desktop input method hosts. */
export function MaintenanceShortcutsSection({
  visible,
  macos,
  linux,
  maintenanceChord,
}: MaintenanceShortcutsSectionProps) {
  if (!visible) return null;

  return (
    <GroupList title={macos ? "输入上下文维护快捷键" : "全局维护快捷键"}>
      <p className={settings.groupNote}>
        {macos
          ? "仅在水杉输入法当前输入上下文生效；Option 对应 Windows 基线中的 Alt。"
          : linux
            ? "在当前 IBus 或 Fcitx5 输入上下文中维护候选与重启服务"
            : "程序运行时全局生效；用于维护与调试"}
      </p>
      <Row title="删除当前候选窗口中的第 1–8 项">{key(`${maintenanceChord}+1–8`)}</Row>
      {linux ? (
        <>
          <Row title="清除当前输入法会话的 Engine 缓存">{key("Ctrl+Shift+Alt+C")}</Row>
          <Row title="重启或重载输入法（IBus 执行 ibus restart，Fcitx5 重置水杉插件，不影响其他输入法）">
            {key("Ctrl+Shift+Alt+R")}
          </Row>
          <Row title={danger("立即退出 IBus 宿主进程（Fcitx5 下与 Fcitx5 同进程，不提供）")}>
            {key("Ctrl+Shift+Alt+T")}
          </Row>
        </>
      ) : (
        <>
          <Row title={macos ? "清除当前输入法会话的 Engine 缓存" : "清除输入法引擎缓存"}>
            {key(`${maintenanceChord}+C`)}
          </Row>
          <Row title={macos ? "重新注册并重启当前输入法" : "重启输入法服务"}>
            {key(`${maintenanceChord}+R`)}
          </Row>
          <Row title={danger(macos ? "立即退出当前输入法进程" : "立即退出输入法服务")}>
            {key(`${maintenanceChord}+T`)}
          </Row>
        </>
      )}
    </GroupList>
  );
}
