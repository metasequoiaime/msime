import type { ConfirmRequest } from "../core/confirm";
import { Row, Switch } from "../core/platform-controls";
import { SettingCheck } from "./setting-check";
import * as settings from "./settings-style";

export type FuzzyPinyinPreferences = { enabled: boolean; rules: string[]; seeded?: boolean };

export const defaultFuzzyPinyin: FuzzyPinyinPreferences = { enabled: false, rules: [] };

const fuzzyPinyinGroups: [string, [string, string][]][] = [
  [
    "平翘舌",
    [
      ["z-zh", "z ↔ zh"],
      ["c-ch", "c ↔ ch"],
      ["s-sh", "s ↔ sh"],
    ],
  ],
  [
    "声母",
    [
      ["n-l", "n ↔ l"],
      ["f-h", "f ↔ h"],
      ["r-l", "r ↔ l"],
    ],
  ],
  [
    "前后鼻音",
    [
      ["an-ang", "an ↔ ang"],
      ["en-eng", "en ↔ eng"],
      ["in-ing", "in ↔ ing"],
    ],
  ],
  [
    "其他韵母",
    [
      ["ian-iang", "ian ↔ iang"],
      ["uan-uang", "uan ↔ uang"],
    ],
  ],
];

const fuzzyPinyinRuleIds = fuzzyPinyinGroups.flatMap(([, rules]) => rules.map(([id]) => id));

export interface FuzzyPinyinSectionProps {
  preferences: FuzzyPinyinPreferences;
  onChange: (preferences: FuzzyPinyinPreferences) => void;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
}

/** Shared fuzzy-pinyin rule controls used by settings hosts that expose the capability: the body of the 拼写纠错 group on the 标点与翻译 page. */
export function FuzzyPinyinSection({ preferences, onChange, confirm }: FuzzyPinyinSectionProps) {
  return (
    <div role="group" aria-label="模糊音" className={settings.rowStack}>
      <Row title="模糊音" description="全拼、九键与双拼均支持；更改会在当前输入结束后生效">
        <Switch
          aria-label="启用模糊音"
          checked={preferences.enabled}
          onChange={(enabled) => {
            const firstEnable = enabled && !preferences.seeded;
            onChange({
              ...preferences,
              enabled,
              ...(firstEnable ? { rules: fuzzyPinyinRuleIds, seeded: true } : {}),
            });
          }}
        />
      </Row>
      <div className={settings.groupBlock}>
        <p className="input-setting-description">
          勾选容易混淆的读音后，会补充对应候选。关闭总开关会保留已选规则。
        </p>
        {fuzzyPinyinGroups.map(([title, rules]) => (
          <div key={title} className="fuzzy-pinyin-group">
            <div className="section-title">{title}</div>
            <div className="input-option-content">
              {rules.map(([id, label], index) => (
                <div className="input-option-item" key={id}>
                  {index > 0 && <div className="input-option-divider" />}
                  <SettingCheck
                    label={label}
                    ariaLabel={`模糊音规则 ${id}`}
                    disabled={!preferences.enabled}
                    checked={preferences.rules.includes(id)}
                    onChange={(checked) => {
                      const selected = new Set(preferences.rules);
                      if (checked) selected.add(id);
                      else selected.delete(id);
                      onChange({ ...preferences, rules: [...selected].sort() });
                    }}
                  />
                </div>
              ))}
            </div>
          </div>
        ))}
        <button
          type="button"
          className="secondary fuzzy-pinyin-reset"
          onClick={() => {
            void confirm({
              title: "关闭模糊音",
              message: "所有模糊音规则会被清空。",
              confirmLabel: "关闭并清空",
              danger: true,
            }).then((confirmed) => {
              if (!confirmed) return;
              onChange({
                enabled: false,
                rules: [],
                seeded: preferences.seeded ?? false,
              });
            });
          }}
        >
          重置模糊音配置
        </button>
      </div>
    </div>
  );
}
