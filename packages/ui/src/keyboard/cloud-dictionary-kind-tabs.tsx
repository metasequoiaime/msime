import * as cloud from "./cloud-panel-style";

export type CloudDictionaryKind = "pinyin" | "wubi" | "wubi98" | "quick" | "english";

export const cloudDictionaryKinds: [CloudDictionaryKind, string][] = [
  ["pinyin", "拼音"],
  ["wubi", "86 五笔"],
  ["wubi98", "98 五笔"],
  ["quick", "快捷短语"],
  ["english", "英文"],
];

export interface CloudDictionaryKindTabsProps {
  value: CloudDictionaryKind;
  disabled?: boolean;
  onChange: (kind: CloudDictionaryKind) => void;
}

/** Shared kind tabs used by the cloud dictionary and file panels. */
export function CloudDictionaryKindTabs({
  value,
  disabled = false,
  onChange,
}: CloudDictionaryKindTabsProps) {
  return (
    <div className={cloud.dictionaryKindTabs} role="tablist" aria-label="云词库类型">
      {cloudDictionaryKinds.map(([kind, label]) => (
        <button
          key={kind}
          type="button"
          role="tab"
          aria-selected={value === kind}
          className={cloud.dictionaryKindTab(value === kind)}
          onClick={() => onChange(kind)}
          disabled={disabled}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
