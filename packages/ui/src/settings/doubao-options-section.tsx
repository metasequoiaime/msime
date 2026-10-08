import { SettingsGroupNote } from "./settings-group-note";
import { TextInputRow } from "./text-input-row";
import { SwitchRow } from "./switch-row";

export interface DoubaoOptionsSectionProps {
  linux: boolean;
  enableItn: boolean;
  enablePunc: boolean;
  enableDdc: boolean;
  boostingTableId: string;
  onEnableItnChange: (enabled: boolean) => void;
  onEnablePuncChange: (enabled: boolean) => void;
  onEnableDdcChange: (enabled: boolean) => void;
  onBoostingTableIdChange: (value: string) => void;
  /** 是否绘制「标点预测」。HarmonyOS 手机在它的「识别」组里以「自动添加标点」显示同一个开关，所以这里不再重复。 */
  punctuation?: boolean;
}

/** 豆包识别的几个开关和热词表，不带组；语音页把它们接在「识别服务配置」组里豆包的设置后面。 */
export function DoubaoOptionsRows({
  linux,
  enableItn,
  enablePunc,
  enableDdc,
  boostingTableId,
  onEnableItnChange,
  onEnablePuncChange,
  onEnableDdcChange,
  onBoostingTableIdChange,
  punctuation = true,
}: DoubaoOptionsSectionProps) {
  return (
    <>
      <SettingsGroupNote>
        {linux ? "以下豆包识别选项由语音服务应用" : "以下豆包识别选项随识别请求发送给豆包"}
      </SettingsGroupNote>
      <SwitchRow
        title="数字格式化"
        aria-label="数字格式化"
        checked={enableItn}
        onChange={onEnableItnChange}
      />
      {punctuation && (
        <SwitchRow
          title="标点预测"
          aria-label="标点预测"
          checked={enablePunc}
          onChange={onEnablePuncChange}
        />
      )}
      <SwitchRow
        title="语义顺滑"
        aria-label="语义顺滑"
        checked={enableDdc}
        onChange={onEnableDdcChange}
      />
      <TextInputRow
        title="热词表 ID"
        label="热词表 ID"
        value={boostingTableId}
        onChange={onBoostingTableIdChange}
      />
    </>
  );
}
