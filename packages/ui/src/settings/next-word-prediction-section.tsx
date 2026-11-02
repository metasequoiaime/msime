import type { NextWordPredictionButtons } from "../core/preferences-model";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

export interface NextWordPredictionSectionProps {
  value?: boolean;
  buttons?: NextWordPredictionButtons;
  /** 触屏宿主的候选栏右端有展开、关闭键可放，给出「联想栏按键」一行；桌面候选窗没有这两个键，不列出。 */
  showButtons: boolean;
  onChange: (change: {
    next_word_prediction?: boolean;
    next_word_prediction_buttons?: NextWordPredictionButtons;
  }) => void;
}

const BUTTONS_DESCRIPTION =
  "联想出现时候选栏右端的按键：× 关闭联想，不输入任何字符；∨ 展开更多联想。只有关闭时左右滑动候选栏选词";

/** 「上屏后联想」开关与联想栏按键样式，文字与 Android 原生的「表达 → 智能」一致。缺省为关；只在声明 `HostCapabilities.next_word_prediction` 的宿主上列出。按键样式只在开关打开时可选。 */
export function NextWordPredictionSection({
  value,
  buttons,
  showButtons,
  onChange,
}: NextWordPredictionSectionProps) {
  const enabled = value ?? false;
  return (
    <>
      <SwitchRow
        title="上屏后联想"
        description="选词上屏后推荐后续文字，点选、按数字键或空格上屏，继续输入其他内容时消失；使用现有词库，开启学习时也参考个人词序，隐私输入不显示"
        checked={enabled}
        onChange={(next_word_prediction) => onChange({ next_word_prediction })}
      />
      {showButtons && (
        <SelectRow
          title="联想栏按键"
          description={BUTTONS_DESCRIPTION}
          value={buttons ?? "expand_only"}
          disabled={!enabled}
          onChange={(event) =>
            onChange({
              next_word_prediction_buttons: event.target.value as NextWordPredictionButtons,
            })
          }
        >
          <option value="expand_only">只有展开 ∨</option>
          <option value="close_and_expand">关闭 × 和展开 ∨</option>
          <option value="close_only">只有关闭 ×</option>
        </SelectRow>
      )}
    </>
  );
}
