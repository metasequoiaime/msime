import { useEffect, useRef, useState } from "react";
import {
  parsePersonalDictionaryImport,
  personalDictionaryExample,
  readDictionaryFile,
  type PersonalDictionaryImportEntry,
} from "../dictionary/dictionary-file";
import * as settings from "./settings-style";

export interface PersonalDictionaryImportClient {
  importPersonal?: (
    text: string,
    requestId: string,
  ) => Promise<{ queued: boolean; pending_count: number }>;
}

export interface PersonalDictionaryImportCardProps {
  dictionary: PersonalDictionaryImportClient;
  platform?: string;
}

function personalDictionaryKindTitle(kind: PersonalDictionaryImportEntry["kind"]): string {
  return kind === "pinyin"
    ? "拼音"
    : kind === "wubi"
      ? "五笔"
      : kind === "quickPhrase"
        ? "快捷短语"
        : "英文";
}

/** Imports an Apple-compatible personal dictionary into the host's sync queue. */
export function PersonalDictionaryImportCard({
  dictionary,
  platform,
}: PersonalDictionaryImportCardProps) {
  const input = useRef<HTMLInputElement>(null);
  const [fileName, setFileName] = useState("");
  const [entries, setEntries] = useState<PersonalDictionaryImportEntry[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const mounted = useRef(true);
  const dictionaryGeneration = useRef(0);

  useEffect(() => {
    mounted.current = true;
    dictionaryGeneration.current++;
    setBusy(false);
    return () => {
      mounted.current = false;
      dictionaryGeneration.current++;
    };
  }, [dictionary]);

  const chooseFile = async (file: File | undefined) => {
    if (!file) return;
    const generation = dictionaryGeneration.current;
    setEntries(null);
    setFileName(file.name);
    setError("");
    setNotice("");
    setBusy(true);
    try {
      // The Apple-compatible personal dictionary file is at most 1 MiB.
      const parsed = parsePersonalDictionaryImport(await readDictionaryFile(file, 1_048_576));
      if (!mounted.current || generation !== dictionaryGeneration.current) return;
      setEntries(parsed);
    } catch (cause) {
      if (mounted.current && generation === dictionaryGeneration.current)
        setError(cause instanceof Error ? cause.message : "无法读取所选文件，请重新选择。");
    } finally {
      if (mounted.current && generation === dictionaryGeneration.current) setBusy(false);
    }
  };

  const importEntries = async () => {
    if (!entries || !dictionary.importPersonal) return;
    const generation = dictionaryGeneration.current;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const text = JSON.stringify({ format: "msime-personal-dictionary", version: 1, entries });
      const result = await dictionary.importPersonal(text, `ui-personal-import-${Date.now()}`);
      if (!mounted.current || generation !== dictionaryGeneration.current) return;
      setNotice(
        `已加入本机同步队列，共 ${entries.length} 条；当前等待同步 ${result.pending_count} 条。`,
      );
      setEntries(null);
      setFileName("");
    } catch (cause) {
      if (mounted.current && generation === dictionaryGeneration.current)
        setError(cause instanceof Error ? cause.message : "导入失败，请稍后重试。");
    } finally {
      if (mounted.current && generation === dictionaryGeneration.current) setBusy(false);
    }
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

  return (
    <div
      className={`section ${settings.importSection}`}
      role="region"
      aria-label="个人词库文件导入"
    >
      <div className="section-header">
        <span className="section-title">
          个人词库文件
          <small>
            导入 Apple 兼容的 JSON 词条，确认后加入
            {platform === "ios" ? " iOS " : platform === "android" ? " Android " : ""}
            键盘同步队列；文件内容不会上传。
          </small>
        </span>
        <span>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => input.current?.click()}
          >
            选择 JSON 文件
          </button>{" "}
          <button type="button" className="secondary" disabled={busy} onClick={saveExample}>
            保存示例文件
          </button>
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
        </span>
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
    </div>
  );
}
