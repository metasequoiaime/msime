import type { ConfirmRequest } from "../core/confirm";
import { SwitchRow } from "./switch-row";
import { SettingCheck } from "./setting-check";
import { SettingSectionTitle } from "./setting-section-title";
import { ActionButton } from "../core/action-button";
import { SettingsInputDescription } from "./settings-input-description";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsRowStack } from "./settings-row-stack";
import { Checks, MoreOptions, Row } from "../core/platform-controls";

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
  /** `row` 是鸿蒙手机的形态：在所在分组（中文）的行里放一个「模糊音」开关，规则对收在它下面的「更多选项」折叠里。 */
  layout?: "section" | "row";
}

/** 有模糊音能力的宿主共用的模糊音设置：输入页「模糊音」组的内容。总开关关闭时收起规则列表和重置按钮，只留总开关；关闭总开关仍保留已选规则，重新打开后原样展开。 */
export function FuzzyPinyinSection({
  preferences,
  onChange,
  confirm,
  layout = "section",
}: FuzzyPinyinSectionProps) {
  const setEnabled = (enabled: boolean) => {
    const firstEnable = enabled && !preferences.seeded;
    onChange({
      ...preferences,
      enabled,
      ...(firstEnable ? { rules: fuzzyPinyinRuleIds, seeded: true } : {}),
    });
  };
  const setRule = (id: string, checked: boolean) => {
    const selected = new Set(preferences.rules);
    if (checked) selected.add(id);
    else selected.delete(id);
    onChange({ ...preferences, rules: [...selected].sort() });
  };
  const reset = () => {
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
  };
  if (layout === "row")
    return (
      <>
        <SwitchRow
          title="模糊音"
          aria-label="启用模糊音"
          checked={preferences.enabled}
          onChange={setEnabled}
        />
        {/* 折叠区是分组里的一行，所以开关关闭时由一个隐藏的包裹元素把它连同细线一起拿掉。 */}
        <div className="min-w-0" hidden={!preferences.enabled}>
          <MoreOptions>
            {fuzzyPinyinGroups.map(([title, rules]) => (
              // 说明文字落在行首内边距上，读起来是下方两列规则对列表的标题。
              <div
                key={title}
                className="pt-2.5 [&>fieldset>legend]:px-4 [&>fieldset>legend]:[font-size:var(--p-sub-fs)] [&>fieldset>legend]:[color:var(--p-sub)]"
              >
                <Checks
                  legend={title}
                  layout="grid"
                  items={rules.map(([id, label]) => ({
                    value: id,
                    label,
                    checked: preferences.rules.includes(id),
                  }))}
                  onChange={setRule}
                />
              </div>
            ))}
            <Row title="重置模糊音配置" description="关闭模糊音并清空所有规则">
              <ActionButton
                className="secondary"
                action={reset}
                ariaLabel="重置模糊音配置"
                label="重置"
              />
            </Row>
          </MoreOptions>
        </div>
      </>
    );
  return (
    <SettingsRowStack role="group" aria-label="模糊音">
      <SwitchRow
        title="启用模糊音"
        description="全拼、九键与双拼均支持；更改会在当前输入结束后生效"
        aria-label="启用模糊音"
        checked={preferences.enabled}
        onChange={setEnabled}
      />
      <SettingsGroupBlock hidden={!preferences.enabled}>
        <SettingsInputDescription>
          勾选容易混淆的读音后，会补充对应候选。关闭总开关会保留已选规则。
        </SettingsInputDescription>
        {fuzzyPinyinGroups.map(([title, rules]) => (
          <div key={title} className="fuzzy-pinyin-group">
            <SettingSectionTitle as="div" title={title} />
            <div className="input-option-content">
              {rules.map(([id, label], index) => (
                <div className="input-option-item" key={id}>
                  {index > 0 && <div className="input-option-divider" />}
                  <SettingCheck
                    label={label}
                    ariaLabel={`模糊音规则 ${id}`}
                    disabled={!preferences.enabled}
                    checked={preferences.rules.includes(id)}
                    onChange={(checked) => setRule(id, checked)}
                  />
                </div>
              ))}
            </div>
          </div>
        ))}
        <ActionButton
          className="secondary fuzzy-pinyin-reset"
          action={reset}
          label="重置模糊音配置"
        />
      </SettingsGroupBlock>
    </SettingsRowStack>
  );
}
