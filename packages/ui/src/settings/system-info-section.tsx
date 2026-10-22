import type { HostCapabilities } from "../core/host-capabilities";

import { GroupList, Row } from "../core/platform-controls";
import { useTemporaryFlag } from "../core/use-temporary-flag";
import { ActionButton } from "./action-button";
import * as doc from "./document-style";
import { platformOsName } from "./label-helpers";
import { useMountedRef } from "./use-mounted-ref";

export interface SystemInfoSource {
  appVersion: string;
  /** 发布日期 `YYYY-MM-DD`，不知道时为空或不给。 */
  releaseDate?: string;
  host?: Pick<
    HostCapabilities,
    | "platform"
    | "os_version"
    | "arch"
    | "kernel_version"
    | "desktop_session"
    | "input_method_framework"
    | "device_model"
  > & { edition?: { display_name?: string } };
  /** 当前输入方案的显示名，例如「全拼」。 */
  scheme?: string;
}

export interface SystemInfoEntry {
  label: string;
  value: string;
}

/**
 * 「关于」页「系统信息」列出的各项，也是「复制」出去的内容（#6644）：应用版本（带发布日期）、系统、内核、桌面会话、输入法框架、设备型号、处理器架构和输入方案。宿主报告不了的项直接省略，不写「未知」。只描述机器、会话和输入法设置，不含账号、输入内容或任何路径。
 */
export function systemInfoEntries({
  appVersion,
  releaseDate,
  host,
  scheme,
}: SystemInfoSource): SystemInfoEntry[] {
  // 版本号后的括注与关于页「当前版本」一致，非完整版另写版本名：`v0.53.0（水杉五笔，2026-10-19 发布）`。
  const notes = [host?.edition?.display_name, releaseDate && `${releaseDate} 发布`].filter(
    (note): note is string => !!note,
  );
  const os = host ? platformOsName(host.platform) : undefined;
  const fields: [string, string | undefined][] = [
    [
      "应用版本",
      appVersion ? `v${appVersion}${notes.length ? `（${notes.join("，")}）` : ""}` : undefined,
    ],
    ["系统", os && (host?.os_version ? `${os} ${host.os_version}` : os)],
    ["内核", host?.kernel_version],
    ["桌面环境", host?.desktop_session],
    ["输入法框架", host?.input_method_framework],
    ["设备型号", host?.device_model],
    ["处理器架构", host?.arch],
    ["输入方案", scheme],
  ];
  return fields.flatMap(([label, value]) => (value ? [{ label, value }] : []));
}

/** 「复制」放进剪贴板的文本：一行标题，然后每项一行「名称：值」，贴进问题报告里直接可读。 */
export function systemInfoText(entries: readonly SystemInfoEntry[]): string {
  return ["水杉 IME 系统信息", ...entries.map(({ label, value }) => `${label}：${value}`)].join(
    "\n",
  );
}

export interface SystemInfoSectionProps extends SystemInfoSource {
  /** 宿主能写剪贴板时才给；没有时只列出各项，不画「复制」按钮。 */
  copyText?: (text: string) => Promise<void>;
  setError?: (message: string) => void;
}

/** 「关于」页的「系统信息」分组：逐项列出，最后一行一键复制，反馈问题时贴进报告。 */
export function SystemInfoSection({ copyText, setError, ...source }: SystemInfoSectionProps) {
  const mounted = useMountedRef();
  const [copied, showCopied] = useTemporaryFlag(1600);
  const entries = systemInfoEntries(source);
  const copy = copyText
    ? async () => {
        if (!mounted.current) return;
        setError?.("");
        try {
          await copyText(systemInfoText(entries));
          if (mounted.current) showCopied();
        } catch {
          if (mounted.current) setError?.("复制系统信息失败，请重试。");
        }
      }
    : undefined;
  return (
    <GroupList title="系统信息">
      {entries.map(({ label, value }) => (
        <Row key={label} title={label}>
          <span className={doc.systemInfoValue}>{value}</span>
        </Row>
      ))}
      {copy && (
        <Row title="复制系统信息" description="反馈问题时贴进去，方便排查；不含账号和输入内容">
          <ActionButton
            action={copy}
            className={doc.rowButton}
            ariaLabel="复制系统信息"
            label={copied ? "已复制" : "复制"}
          />
        </Row>
      )}
    </GroupList>
  );
}
