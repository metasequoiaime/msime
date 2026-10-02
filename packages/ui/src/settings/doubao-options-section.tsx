import * as settings from "./settings-style";
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
}: DoubaoOptionsSectionProps) {
  return (
    <>
      <p className={settings.groupNote}>
        {linux ? "以下豆包识别选项由语音服务应用" : "以下豆包识别选项随识别请求发送给豆包"}
      </p>
      <SwitchRow
        title="数字格式化"
        aria-label="数字格式化"
        checked={enableItn}
        onChange={onEnableItnChange}
      />
      <SwitchRow
        title="标点预测"
        aria-label="标点预测"
        checked={enablePunc}
        onChange={onEnablePuncChange}
      />
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
