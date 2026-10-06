import { SettingsRowStack } from "./settings-row-stack";
import { GroupList } from "../core/platform-controls";
import { ActionRow } from "./action-row";
import { SwitchRow } from "./switch-row";

export interface DiagnosticLogPreferences {
  server: boolean;
  tsf: boolean;
}

export interface DiagnosticLogsSectionProps {
  visible: boolean;
  linux: boolean;
  macos: boolean;
  windows: boolean;
  values: DiagnosticLogPreferences;
  openDirectory?: () => Promise<void>;
  onChange: (patch: Partial<DiagnosticLogPreferences>) => void;
  onError: (message: string) => void;
}

/** Diagnostic log toggles and log-directory action shared by desktop hosts. */
export function DiagnosticLogsSection({
  visible,
  linux,
  macos,
  windows,
  values,
  openDirectory,
  onChange,
  onError,
}: DiagnosticLogsSectionProps) {
  if (!visible) return null;

  const title = linux ? "输入法宿主日志" : macos ? "输入法日志" : "Server 端日志";
  const description = linux
    ? "排查 IBus 或 Fcitx5 宿主的焦点切换、设置应用和菜单保存问题时开启。记录焦点进出、偏好应用、菜单保存、词库维护时释放会话的结果和操作失败的阶段，限量轮转，不记录按键、输入内容或候选文本。文件是数据目录下的 diagnostic.log，两个宿主写进同一个文件，复现后可直接发送。"
    : macos
      ? "排查按键延迟、候选窗口位置、焦点切换和设置加载失败时开启。记录焦点进出，偏好加载、应用、保存的结果，超过 8 毫秒的按键处理耗时，候选窗口的显示位置与隐藏原因，以及输入统计写入失败的类别，限量轮转，不记录按键、输入内容或候选文本。文件是应用支持目录下的 diagnostic.log，复现后用「在 Finder 中显示」找到它并发送。"
      : "排查 Server 启动和通信问题时开启。记录 Server 启停原因和各组件是否就绪，限量轮转，不记录按键、输入内容或候选文本。文件是数据目录下的 logs\\server.log，TSF 端日志也写进这个文件，复现后可直接发送。";

  async function revealDirectory() {
    if (!openDirectory) return;
    onError("");
    try {
      await openDirectory();
    } catch {
      onError(
        macos
          ? "无法在 Finder 中显示诊断日志，请稍后重试。"
          : "无法打开日志目录，可能是文件管理器不可用。",
      );
    }
  }

  return (
    <GroupList title="诊断日志">
      <SettingsRowStack role="group" aria-label="诊断日志">
        <SwitchRow
          title={title}
          description={description}
          checked={values.server}
          onChange={(server) => onChange({ server })}
        />
        {openDirectory && (
          <ActionRow
            title="日志文件"
            description={
              macos
                ? "在 Finder 中选中 diagnostic.log；还没有写入时打开它所在的目录。"
                : "打开日志文件所在的目录。"
            }
            action={revealDirectory}
            label={macos ? "在 Finder 中显示" : "打开日志目录"}
          />
        )}
        {!linux && windows && (
          <SwitchRow
            title="TSF 端日志"
            description="排查应用内预编辑和输入延迟时开启。日志在内存中限量缓冲，并通过独立管道批量汇总，不记录按键、输入内容或候选文本。"
            checked={values.tsf}
            onChange={(tsf) => onChange({ tsf })}
          />
        )}
      </SettingsRowStack>
    </GroupList>
  );
}
