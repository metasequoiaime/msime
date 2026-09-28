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
    <div className="section">
      <div className="section-title">
        豆包识别选项
        <small>{linux ? "由 provider 服务应用" : "随识别请求发送给豆包"}</small>
      </div>
      <SettingToggle
        label="数字格式化"
        ariaLabel="数字格式化"
        checked={enableItn}
        compact
        onChange={onEnableItnChange}
      />
      <SettingToggle
        label="标点预测"
        ariaLabel="标点预测"
        checked={enablePunc}
        compact
        onChange={onEnablePuncChange}
      />
      <SettingToggle
        label="语义顺滑"
        ariaLabel="语义顺滑"
        checked={enableDdc}
        compact
        onChange={onEnableDdcChange}
      />
      <label className="section-header">
        <span className="section-title">热词表 ID</span>
        <input
          aria-label="热词表 ID"
          value={boostingTableId}
          onChange={(event) => onBoostingTableIdChange(event.target.value)}
        />
      </label>
    </div>
  );
}
import { SettingToggle } from "./setting-toggle";
