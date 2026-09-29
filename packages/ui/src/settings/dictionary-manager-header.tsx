import type { LocalDictionaryFormat } from "../dictionary/dictionary-file";
import * as settings from "./settings-style";

export interface DictionaryManagerHeaderProps {
  disabled: boolean;
  dictionaryFormat: LocalDictionaryFormat;
  onQuery: () => void;
  onAdd: () => void;
  onExportCurrent: () => void;
  onExportAll: () => void;
  onImport: (file: File) => void;
  onFormatChange: (format: LocalDictionaryFormat) => void;
}

/** Header actions for the local Engine dictionary manager. */
export function DictionaryManagerHeader({
  disabled,
  dictionaryFormat,
  onQuery,
  onAdd,
  onExportCurrent,
  onExportAll,
  onImport,
  onFormatChange,
}: DictionaryManagerHeaderProps) {
  return (
    <div className={`section ${settings.managerHeader}`} role="region" aria-label="快捷短语管理">
      <div className="section-header">
        <span className="section-title">
          本地词库管理
          <small>
            查询、新增、编辑、导入、导出和删除 Engine 用户词库。导入支持标准、Windows TSV、Rime
            和纯汉字自动注音。标有「内置」的是随输入法附带的词条，只能调整权重或删除。
          </small>
        </span>
        <span>
          <button type="button" className="secondary" disabled={disabled} onClick={onQuery}>
            查询
          </button>{" "}
          <button type="button" className="secondary" disabled={disabled} onClick={onAdd}>
            新增词条
          </button>{" "}
          <button
            type="button"
            className="secondary"
            disabled={disabled || dictionaryFormat === "hans"}
            onClick={onExportCurrent}
          >
            导出当前类型
          </button>{" "}
          <button type="button" className="secondary" disabled={disabled} onClick={onExportAll}>
            导出全部
          </button>
          <label className="secondary">
            导入
            <input
              hidden
              type="file"
              accept=".txt,.tsv,.yaml,.yml,text/plain"
              disabled={disabled}
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) {
                  const name = file.name.toLowerCase();
                  if (name.endsWith(".yaml") || name.endsWith(".yml")) onFormatChange("rime");
                  onImport(file);
                }
                event.currentTarget.value = "";
              }}
            />
          </label>
        </span>
      </div>
    </div>
  );
}
