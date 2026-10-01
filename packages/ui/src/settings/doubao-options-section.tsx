import { GroupList, Row, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";

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

/** Doubao recognition flags and hotword table settings. */
export function DoubaoOptionsSection({
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
    <GroupList title="豆包识别选项">
      <p className={settings.groupNote}>
        {linux ? "由 provider 服务应用" : "随识别请求发送给豆包"}
      </p>
      <Row title="数字格式化">
        <Switch aria-label="数字格式化" checked={enableItn} onChange={onEnableItnChange} />
      </Row>
      <Row title="标点预测">
        <Switch aria-label="标点预测" checked={enablePunc} onChange={onEnablePuncChange} />
      </Row>
      <Row title="语义顺滑">
        <Switch aria-label="语义顺滑" checked={enableDdc} onChange={onEnableDdcChange} />
      </Row>
      <Row title="热词表 ID">
        <input
          aria-label="热词表 ID"
          value={boostingTableId}
          onChange={(event) => onBoostingTableIdChange(event.target.value)}
        />
      </Row>
    </GroupList>
  );
}
