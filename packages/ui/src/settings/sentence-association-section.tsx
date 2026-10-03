import type { SentenceAssociationPreferences } from "../index";
import { SwitchRow } from "./switch-row";

export interface SentenceAssociationSectionProps {
  value: SentenceAssociationPreferences | undefined;
  /** 触屏宿主写 `neural_keyboard`，桌面宿主写 `neural_desktop`，行名也随之改为「键盘」或「桌面」。 */
  mobile: boolean;
  /** 本版本带键盘神经联想的模型（`HostCapabilities.edition.neural_keyboard`）；为 false 时触屏宿主不列出神经联想开关。缺省为 true。 */
  neuralKeyboard?: boolean;
  onChange: (value: SentenceAssociationPreferences) => void;
}

/** 本地整句联想与神经联想两个开关：输入页「候选与联想」组里的两行。 */
export function SentenceAssociationSection({
  value,
  mobile,
  neuralKeyboard = true,
  onChange,
}: SentenceAssociationSectionProps) {
  return (
    <>
      <SwitchRow
        title="本地整句联想"
        description="把词库组合出的整句加入候选；关闭后仍保留单词候选。"
        checked={value?.word_lattice ?? true}
        onChange={(checked) => onChange({ ...value, word_lattice: checked })}
      />
      {(!mobile || neuralKeyboard) && (
        <SwitchRow
          title={mobile ? "键盘神经联想" : "桌面神经联想"}
          description="使用随包的神经模型重排整句候选；没有模型时保持现有候选。"
          checked={mobile ? (value?.neural_keyboard ?? false) : (value?.neural_desktop ?? false)}
          onChange={(checked) =>
            onChange({
              ...value,
              ...(mobile ? { neural_keyboard: checked } : { neural_desktop: checked }),
            })
          }
        />
      )}
    </>
  );
}
