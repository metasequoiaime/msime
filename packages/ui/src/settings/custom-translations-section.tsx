import { Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import type { SettingsSaveState } from "./use-settings-persistence";

export interface CustomTranslationsSectionProps {
  mobile: boolean;
  value: string;
  placeholder: string;
  notice: string;
  summary: string;
  /** Where the automatic save of the overlay stands. */
  saveState: SettingsSaveState;
  /** Why the last save failed, shown while `saveState` is `failed`. */
  saveError: string;
  onChange: (value: string) => void;
  /** Writes the pending edit now instead of waiting out the countdown; also the 重试 after a failure. */
  onFlush: () => void;
}

/** Editor for user supplied candidate translation overrides; edits are saved automatically. */
export function CustomTranslationsSection({
  mobile,
  value,
  placeholder,
  notice,
  summary,
  saveState,
  saveError,
  onChange,
  onFlush,
}: CustomTranslationsSectionProps) {
  return (
    <div role="group" aria-label="自定义候选释义设置" className={settings.rowStack}>
      <Row
        title="自定义候选释义"
        description={`${mobile ? "候选栏" : "候选窗口"}的中英互译来自内置词库；覆盖不全或译得不准时，可以自己加一层，不改内置词库。每行一条，用 Tab 分隔源词和译文；以 # 开头的行是注释。源词含汉字即为中译英，全是英文则为英译中。同一个源词写多次时以最后一次为准。修改会自动保存，重新启动输入法后生效。`}
      />
      <div className={settings.groupBlock}>
        <textarea
          aria-label="自定义候选释义"
          rows={8}
          value={value}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
          onBlur={onFlush}
        />
        <p className={settings.customGlossesStatus}>
          <span role="status">{notice || summary}</span>
          {saveState === "failed" ? (
            <>
              <span role="alert">{saveError}</span>
              <button type="button" className="secondary" onClick={onFlush}>
                重试
              </button>
            </>
          ) : (
            <span aria-live="polite">
              {saveState === "saving" ? "正在保存…" : saveState === "saved" ? "已保存" : ""}
            </span>
          )}
        </p>
      </div>
    </div>
  );
}
