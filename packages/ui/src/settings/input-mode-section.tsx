import { Row, Segmented } from "../core/platform-controls";

export type InputModeScheme = "quanpin" | "shuangpin" | "wubi" | "japanese";

export interface InputModeSectionProps {
  scheme: InputModeScheme;
  lastChineseScheme?: Exclude<InputModeScheme, "japanese"> | null;
  onChange: (
    patch: Partial<{
      scheme: InputModeScheme;
      last_chinese_scheme: Exclude<InputModeScheme, "japanese"> | null;
    }>,
  ) => void;
  /** Hosts that choose among touch keyboard schemes instead keep this row out of sight. */
  hidden?: boolean;
}

const inputModeOptions = [
  { value: "chinese", label: "中文" },
  { value: "japanese", label: "日文" },
] as const;

/** Chinese/Japanese input mode selector with remembered Chinese scheme: the first row of the 方案 group. */
export function InputModeSection({
  scheme,
  lastChineseScheme,
  onChange,
  hidden,
}: InputModeSectionProps) {
  return (
    <Row
      title="输入模式"
      description="切换中文或日文输入，并保留各模式上次选择的方案"
      hidden={hidden}
    >
      <Segmented
        options={inputModeOptions}
        value={scheme === "japanese" ? "japanese" : "chinese"}
        onChange={(mode) =>
          onChange(
            mode === "japanese"
              ? {
                  last_chinese_scheme: scheme === "japanese" ? lastChineseScheme : scheme,
                  scheme: "japanese",
                }
              : { scheme: lastChineseScheme ?? "quanpin" },
          )
        }
      />
    </Row>
  );
}
