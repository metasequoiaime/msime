import { SelectRow } from "./select-row";

export interface WubiPhraseLearningSectionProps {
  placement?: "bottom" | "top";
  onChange: (change: {
    wubi_phrase_placement?: "bottom" | "top";
  }) => void;
}

/** 五笔自造词的落库位置，「候选与联想」组里学习开关之后的一行。仅五笔方案且学习开启时由调用方渲染；缺字段按默认（`bottom`）显示，与引擎和 client-core 一致。 */
export function WubiPhraseLearningSection({
  placement,
  onChange,
}: WubiPhraseLearningSectionProps) {
  return (
    <SelectRow
      title="五笔自造词位置"
      description="新记的五笔词排在候选里的初始位置：置顶排在同码已有候选之前，沉底排在之后；之后选得多了照常按使用习惯调整"
      value={placement ?? "bottom"}
      onChange={(event) =>
        onChange({
          wubi_phrase_placement: event.target.value as "bottom" | "top",
        })
      }
    >
      <option value="bottom">沉底</option>
      <option value="top">置顶</option>
    </SelectRow>
  );
}
