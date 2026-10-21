import { SwitchRow } from "./switch-row";

export interface CandidateExpandSectionProps {
  value?: boolean;
  showButton?: boolean;
  onShowButtonChange: (value: boolean) => void;
  onChange: (value: boolean) => void;
}

/** 「展开候选」开关，供按 `expand_candidates` 画多行候选网格的宿主使用（`HostCapabilities::expand_candidates`：macOS，以及 X11 上用原生画布画候选窗的 Linux Fcitx5）：「候选窗口」页「更多选项」里的一行。默认关闭，所以缺值读成关。 */
export function CandidateExpandSection({
  value,
  onChange,
  showButton,
  onShowButtonChange,
}: CandidateExpandSectionProps) {
  return (
    <>
      <SwitchRow
        title="展开候选"
        description="横排候选窗按 Tab、↓ 或点展开按钮，把后面几页候选排成多行一起显示；竖排候选窗不受影响。"
        checked={value === true}
        onChange={onChange}
      />
      {value === true && (
        <SwitchRow
          title="显示展开键"
          description="独立于页码和翻页按钮显示；关闭后仍可按 Tab 或 ↓ 展开候选。"
          checked={showButton !== false}
          onChange={onShowButtonChange}
        />
      )}
    </>
  );
}
