import { SelectRow } from "./select-row";

export interface WubiPhraseLearningSectionProps {
  learning?: "thrice" | "once";
  placement?: "bottom" | "top";
  onChange: (change: {
    wubi_phrase_learning?: "thrice" | "once";
    wubi_phrase_placement?: "bottom" | "top";
  }) => void;
}

/** 五笔自造词的时机与落库位置，「候选与联想」组里学习开关之后的两行。仅五笔方案且学习开启时由调用方渲染；缺字段按默认（thrice / bottom）显示，与引擎和 client-core 一致。 */
export function WubiPhraseLearningSection({
  learning,
  placement,
  onChange,
}: WubiPhraseLearningSectionProps) {
  return (
    <>
      <SelectRow
        title="五笔造词时机"
        description="连续打出同一串字达到次数后自动记成五笔词组；打 1 次就记更快，凑 3 次才记更不容易把偶然的输入记成词"
        value={learning ?? "thrice"}
        onChange={(event) =>
          onChange({
            wubi_phrase_learning: event.target.value as "thrice" | "once",
          })
        }
      >
        <option value="thrice">凑 3 次才记</option>
        <option value="once">打 1 次就记</option>
      </SelectRow>
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
    </>
  );
}
