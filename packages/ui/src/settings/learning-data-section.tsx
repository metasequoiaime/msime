import { SettingActionHeader } from "./setting-action-header";
import { ActionButton } from "./action-button";

export interface LearningDataSectionProps {
  disabled: boolean;
  onReset: () => void;
}

/** macOS-only controls for clearing the input method's learned data. */
export function LearningDataSection({ disabled, onReset }: LearningDataSectionProps) {
  return (
    <div className="section" role="region" aria-label="学习数据">
      <SettingActionHeader
        title="学习数据"
        description="清除候选词频、用户词库和拼音学习记录；自己新增和修改的词条也会删除，输入方案与其他设置不会改变。"
      >
        <ActionButton
          action={onReset}
          className="secondary danger-button"
          disabled={disabled}
          label="清除全部学习数据"
        />
      </SettingActionHeader>
    </div>
  );
}
