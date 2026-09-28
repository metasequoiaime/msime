export type HelpcodeSchema =
  | "lantian"
  | "ziranma"
  | "shouyou2_0"
  | "shouyouplus"
  | "xiaohe"
  | "jiajia";

export type HelpcodePreferences = {
  enabled: boolean;
  schema: HelpcodeSchema;
  show_in_candidate_window?: boolean;
};

type HelpcodeKey = "quanpin_helpcode" | "shuangpin_helpcode";
export type HelpcodeSettings = Partial<Record<HelpcodeKey, HelpcodePreferences>>;

const defaultHelpcode: Record<HelpcodeKey, HelpcodePreferences> = {
  quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: false },
  shuangpin_helpcode: { enabled: true, schema: "lantian", show_in_candidate_window: true },
};

const helpcodeSchemas: readonly (readonly [HelpcodeSchema, string])[] = [
  ["lantian", "蓝天小雨点"],
  ["ziranma", "自然码"],
  ["shouyou2_0", "首右2.0"],
  ["shouyouplus", "首右plus"],
  ["xiaohe", "小鹤"],
  ["jiajia", "加加"],
];

export interface HelpcodeSettingsPageProps {
  value: HelpcodeSettings;
  mobile: boolean;
  showShiftEntry: boolean;
  disabled?: boolean;
  hidden?: boolean;
  onChange: (patch: HelpcodeSettings) => void;
}

/** The shared helper-code settings form used by desktop and mobile hosts. */
export function HelpcodeSettingsPage({
  value: draft,
  mobile,
  showShiftEntry,
  disabled = false,
  hidden = false,
  onChange,
}: HelpcodeSettingsPageProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="辅助码">
      {showShiftEntry && (
        <div className="section input-setting-description">
          <p>
            全拼或双拼组字时，按 Shift
            再输入的字母作为辅助码交给输入引擎，用于缩小候选。五笔、日语和本地输入模式不使用辅助码。
          </p>
        </div>
      )}
      {(["shuangpin_helpcode", "quanpin_helpcode"] as const).map((key) => {
        const label = key === "shuangpin_helpcode" ? "双拼" : "全拼";
        const current = {
          ...defaultHelpcode[key],
          ...draft[key],
        } as Required<HelpcodePreferences>;
        return (
          <div className="section" key={key}>
            <label className="section-header">
              <span className="section-title">{label}辅助码</span>
              <input
                className="toggle"
                type="checkbox"
                checked={current.enabled}
                onChange={(event) =>
                  onChange({
                    [key]: { ...current, enabled: event.target.checked },
                  })
                }
              />
            </label>
            <label className="section-header helpcode-schema">
              <span className="section-title">{label}辅助码方案</span>
              <select
                disabled={!current.enabled}
                value={current.schema}
                onChange={(event) =>
                  onChange({
                    [key]: {
                      ...current,
                      schema: event.target.value as HelpcodeSchema,
                    },
                  })
                }
              >
                {helpcodeSchemas.map(([schema, name]) => (
                  <option key={schema} value={schema}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <label className="section-header">
              {/* Each scheme has its own accessible name because both rows are visible together. */}
              <span className="section-title">
                {mobile ? `在候选栏中显示${label}辅助码` : `在候选窗口中显示${label}辅助码`}
              </span>
              <input
                aria-label={
                  mobile ? `在候选栏中显示${label}辅助码` : `在候选窗口中显示${label}辅助码`
                }
                className="toggle"
                type="checkbox"
                checked={current.show_in_candidate_window}
                onChange={(event) =>
                  onChange({
                    [key]: {
                      ...current,
                      show_in_candidate_window: event.target.checked,
                    },
                  })
                }
              />
            </label>
          </div>
        );
      })}
    </fieldset>
  );
}
