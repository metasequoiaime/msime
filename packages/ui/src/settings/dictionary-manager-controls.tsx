import type { LocalDictionaryFormat, LocalDictionaryKind } from "../dictionary/dictionary-file";
import { localDictionaryKinds } from "../dictionary/dictionary-kinds";
import * as settings from "./settings-style";

export interface DictionaryManagerControlsProps {
  kind: LocalDictionaryKind;
  format: LocalDictionaryFormat;
  search: string;
  disabled: boolean;
  onKindChange: (kind: LocalDictionaryKind) => void;
  onFormatChange: (format: LocalDictionaryFormat) => void;
  onSearchChange: (search: string) => void;
}

/** Dictionary type, import format, and prefix query controls. */
export function DictionaryManagerControls({
  kind,
  format,
  search,
  disabled,
  onKindChange,
  onFormatChange,
  onSearchChange,
}: DictionaryManagerControlsProps) {
  return (
    <div className={settings.managerControls}>
      <label>
        词库{" "}
        <select
          aria-label="本地词库类型"
          value={kind}
          disabled={disabled}
          onChange={(event) => onKindChange(event.target.value as LocalDictionaryKind)}
        >
          {localDictionaryKinds.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <label>
        文件格式{" "}
        <select
          aria-label="本地词库文件格式"
          value={format}
          disabled={disabled}
          onChange={(event) => onFormatChange(event.target.value as LocalDictionaryFormat)}
        >
          <option value="standard">词在前（标准 TSV）</option>
          <option value="windows">编码在前（Windows TSV）</option>
          <option value="rime">Rime userdb / dict.yaml</option>
          {kind === "pinyin" && <option value="hans">汉字自动注音（仅导入）</option>}
        </select>
      </label>
      <label>
        编码前缀{" "}
        <input
          value={search}
          placeholder="留空查看全部"
          onChange={(event) => onSearchChange(event.target.value)}
        />
      </label>
    </div>
  );
}
