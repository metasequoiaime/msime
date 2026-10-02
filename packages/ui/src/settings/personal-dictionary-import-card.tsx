import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
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
import { useMountedRef } from "./use-mounted-ref";
import { ActionButton } from "./action-button";

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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const mounted = useMountedRef();
  const dictionaryGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    dictionaryGeneration.current++;
    actionRunning.current = false;
    setBusy(false);
    return () => {
      actionRunning.current = false;
      dictionaryGeneration.current++;
    };
  }, [dictionary]);

  async function runDictionaryAction(
    operation: (isCurrent: () => boolean) => Promise<void>,
    formatError: (error: unknown) => string,
  ) {
    if (actionRunning.current || !mounted.current) return;
    actionRunning.current = true;
    const generation = dictionaryGeneration.current;
    try {
      await runAsyncAction(
        {
          busy: false,
          isCurrent: () => mounted.current && generation === dictionaryGeneration.current,
          setBusy,
          setError,
          setNotice,
        },
        operation,
        { formatError },
      );
    } finally {
      actionRunning.current = false;
    }
  }

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
      (cause) => (cause instanceof Error ? cause.message : "无法读取所选文件，请重新选择。"),
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
      (cause) => (cause instanceof Error ? cause.message : "导入失败，请稍后重试。"),
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
      <div className={settings.managerActions}>
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
      </div>
      {busy && <p role="status">正在读取或加入同步队列…</p>}
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
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className="notice">
          {notice}
        </p>
      )}
      {entries && (
        <button
          type="button"
          className="primary"
          disabled={busy}
          onClick={() => void importEntries()}
        >
          确认导入
        </button>
      )}
    </>
  );
  if (embedded) {
    return (
      <div className={settings.managerBlock} role="group" aria-label="个人词库文件">
        <div>
          <span className={rowTitle} data-row-title="">
            个人词库文件
          </span>
          <p className={settings.managerNote}>{note}</p>
        </div>
        {content}
      </div>
    );
  }
  return (
    <GroupList title="个人词库文件">
      <p className={settings.groupNote}>{note}</p>
      <div className={settings.managerBlock}>{content}</div>
    </GroupList>
  );
}
