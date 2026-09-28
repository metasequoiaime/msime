export interface CustomTranslationsSectionProps {
  mobile: boolean;
  value: string;
  placeholder: string;
  notice: string;
  summary: string;
  busy: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}

/** Editor for user supplied candidate translation overrides. */
export function CustomTranslationsSection({
  mobile,
  value,
  placeholder,
  notice,
  summary,
  busy,
  onChange,
  onSave,
}: CustomTranslationsSectionProps) {
  return (
    <div className="section" role="group" aria-label="自定义候选释义设置">
      <div className="section-title">
        自定义候选释义
        <small>
          {mobile ? "候选栏" : "候选窗"}
          的中英互译来自内置词库；覆盖不全或译得不准时，可以自己加一层，不改内置词库。每行一条，用
          Tab 分隔源词和译文；以 #
          开头的行是注释。源词含汉字即为中译英，全是英文则为英译中。同一个源词写多次时以最后一次为准。保存后重新启动输入法生效。
        </small>
      </div>
      <textarea
        aria-label="自定义候选释义"
        rows={8}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
      <p role="status">{notice || summary}</p>
      <button type="button" className="secondary" disabled={busy} onClick={onSave}>
        {busy ? "保存中…" : "保存自定义释义"}
      </button>
    </div>
  );
}
