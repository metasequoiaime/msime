import { GroupList } from "../core/platform-controls";
import { SettingsRowStack } from "./settings-row-stack";
import { SwitchRow } from "./switch-row";

export interface CommandLineToolsSectionProps {
  visible: boolean;
  /** Windows 上命令以 `msime-mcp.exe` 安装，没有 macOS 与 Linux 包里的 `msime` 链接。 */
  windows: boolean;
  candidates: boolean;
  onCandidatesChange: (enabled: boolean) => void;
}

/** 维护与诊断页的「命令行工具」组：只放会把本机数据交给终端命令的开关，由随包带命令行工具的桌面宿主显示。 */
export function CommandLineToolsSection({
  visible,
  windows,
  candidates,
  onCandidatesChange,
}: CommandLineToolsSectionProps) {
  if (!visible) return null;
  const command = windows ? "msime-mcp candidates" : "msime candidates";
  return (
    <GroupList title="命令行工具">
      <SettingsRowStack role="group" aria-label="命令行工具">
        <SwitchRow
          title="命令行查询候选"
          description={`允许终端里的 ${command} 命令按你现在的输入方案、设置和词库，输出一串拼音或按键会给出的候选，供编写测试和演示程序使用。词库里有你打过的词，任何能运行这个命令的程序都能借此读到它们，不用时请保持关闭；关闭时命令会提示并以错误码退出。`}
          checked={candidates}
          onChange={onCandidatesChange}
        />
      </SettingsRowStack>
    </GroupList>
  );
}
