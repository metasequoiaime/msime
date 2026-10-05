import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { useRef, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  parsePersonalDictionaryImport,
  personalDictionaryExample,
  readDictionaryFile,
  type PersonalDictionaryImportEntry,
} from "../dictionary/dictionary-file";
import { GroupList } from "../core/platform-controls";
import { rowTitle } from "../core/platform-controls-style";
import { personalDictionaryKindTitle } from "../dictionary/dictionary-messages";
import * as settings from "./settings-style";
import { SettingsManagerBlock } from "./settings-manager-block";
import { ActionButton } from "./action-button";
import { ErrorAlert } from "../core/error-alert";
import { SettingsNotice } from "./settings-notice";
import { StatusMessage } from "../core/status-message";

export interface PersonalDictionaryImportClient {
  importPersonal?: (
    text: string,
    requestId: string,
  ) => Promise<{ queued: boolean; pending_count: number }>;
}

export interface PersonalDictionaryImportCardProps {
  dictionary: PersonalDictionaryImportClient;
  platform?: string;
  /** 词库页把它并进「导入与导出」组时为真：画成组内带名字的一块（`role="group"`），不再自成一组。 */
  embedded?: boolean;
}

/** Imports an Apple-compatible personal dictionary into the host's sync queue. */
export function PersonalDictionaryImportCard({
  dictionary,
  platform,
  embedded = false,
}: PersonalDictionaryImportCardProps) {
  const input = useRef<HTMLInputElement>(null);
  const [fileName, setFileName] = useState("");
  const [entries, setEntries] = useState<PersonalDictionaryImportEntry[] | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const { busy, run: runDictionaryAction } = useAsyncActionRunner(setError, setNotice, dictionary);

  const chooseFile = async (file: File | undefined) => {
    if (!file || busy) return;
    setEntries(null);
    setFileName(file.name);
    await runDictionaryAction(
      async (isCurrent) => {
        // The Apple-compatible personal dictionary file is at most 1 MiB.
        const parsed = parsePersonalDictionaryImport(await readDictionaryFile(file, 1_048_576));
        if (!isCurrent()) return;
        setEntries(parsed);
      },
      {
        formatError: (cause) =>
          cause instanceof Error ? cause.message : "无法读取所选文件，请重新选择。",
      },
    );
  };

  const importEntries = async () => {
    const importPersonal = dictionary.importPersonal;
    if (!entries || !importPersonal || busy) return;
    await runDictionaryAction(
      async (isCurrent) => {
        const text = JSON.stringify({ format: "msime-personal-dictionary", version: 1, entries });
        const result = await importPersonal(text, `ui-personal-import-${Date.now()}`);
        if (!isCurrent()) return;
        setNotice(
          `已加入本机同步队列，共 ${entries.length} 条；当前等待同步 ${result.pending_count} 条。`,
        );
        setEntries(null);
        setFileName("");
      },
      {
        formatError: (cause) => (cause instanceof Error ? cause.message : "导入失败，请稍后重试。"),
      },
    );
  };

  const saveExample = () => {
    const blob = new Blob([personalDictionaryExample], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "msime-personal-dictionary-example.json";
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const countByKind = entries
    ? Array.from(new Set(entries.map((entry) => entry.kind)))
        .map(
          (kind) =>
            `${personalDictionaryKindTitle(kind)} ${entries.filter((entry) => entry.kind === kind).length} 条`,
        )
        .join(" · ")
    : "";

  const note = (
    <>
      导入 Apple 兼容的 JSON 词条，确认后加入
      {platform === "ios" ? " iOS " : platform === "android" ? " Android " : ""}
      键盘同步队列；文件内容不会上传。
    </>
  );
  const content = (
    <>
      <SettingsManagerActions>
        <ActionButton
          action={() => input.current?.click()}
          disabled={busy}
          label="选择 JSON 文件"
        />
        <ActionButton action={saveExample} disabled={busy} label="保存示例文件" />
        <input
          ref={input}
          hidden
          type="file"
          aria-label="选择个人词库 JSON 文件"
          accept=".json,application/json"
          onChange={(event) => {
            void chooseFile(event.currentTarget.files?.[0]);
            event.currentTarget.value = "";
          }}
        />
      </SettingsManagerActions>
      {busy && <StatusMessage role="status">正在读取或加入同步队列…</StatusMessage>}
      {fileName && entries && (
        <div className={settings.importPreview}>
          <strong>{fileName}</strong>
          <span>
            已校验 {entries.length} 条（{countByKind}），确认后逐条同步。
          </span>
          {entries.map((entry, index) => (
            <div key={`${entry.kind}-${entry.key}-${index}`}>
              <span>{entry.value}</span>
              <code>
                {personalDictionaryKindTitle(entry.kind)} · {entry.key}
              </code>
            </div>
          ))}
        </div>
      )}
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {notice && <SettingsNotice role="status">{notice}</SettingsNotice>}
      {entries && (
        <ActionButton
          action={() => void importEntries()}
          className="primary"
          disabled={busy}
          label="确认导入"
        />
      )}
    </>
  );
  if (embedded) {
    return (
      <SettingsManagerBlock role="group" aria-label="个人词库文件">
        <div>
          <span className={rowTitle} data-row-title="">
            个人词库文件
          </span>
          <SettingsManagerNote>{note}</SettingsManagerNote>
        </div>
        {content}
      </SettingsManagerBlock>
    );
  }
  return (
    <GroupList title="个人词库文件">
      <SettingsGroupNote>{note}</SettingsGroupNote>
      <SettingsManagerBlock>{content}</SettingsManagerBlock>
    </GroupList>
  );
}
