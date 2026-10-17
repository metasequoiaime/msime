import {
  ASR_PROVIDER_DEFAULTS,
  DOUBAO_STREAM_ENDPOINTS,
  doubaoStreamEndpointId,
  findDoubaoStreamEndpoint,
} from "../voice/voice-providers";
import { ControlledSelect } from "../core/controlled-select";
import { EndpointSettingRow } from "./endpoint-setting-row";
import { SelectRow } from "./select-row";
import { useCustomEndpointIntent } from "./use-custom-endpoint-intent";

export interface DoubaoStreamEndpointSectionProps {
  endpoint: string;
  onChange: (endpoint: string) => void;
}

export interface DoubaoStreamEndpointSelectProps {
  endpoint: string;
  onChange: (endpoint: string) => void;
  /** 用户选了「自定义地址」：即使地址还是预设，下拉也停在这一项，下面的地址输入框不收起。 */
  custom?: boolean;
  /** 选中或离开「自定义地址」。选它时不写地址，由调用方露出地址输入框。 */
  onCustomChange?: (custom: boolean) => void;
}

/** 「自定义地址」的选项值。 */
const CUSTOM = "custom";

/** 一个返回 Fragment 的普通函数而不是组件：`SelectRow` 在 HarmonyOS 上只认直接的 `<option>` 和 Fragment 子元素。 */
function doubaoStreamEndpointOptions() {
  return (
    <>
      {DOUBAO_STREAM_ENDPOINTS.map((option) => (
        <option key={option.id} value={option.id}>
          {option.title}
        </option>
      ))}
      <option value={CUSTOM}>自定义地址</option>
    </>
  );
}

function updateDoubaoStreamEndpoint(value: string, onChange: (endpoint: string) => void) {
  const chosen = findDoubaoStreamEndpoint(value);
  if (chosen) onChange(chosen.endpoint);
}

/** Shared preset select used by the standalone and credential settings sections. */
export function DoubaoStreamEndpointSelect({
  endpoint,
  onChange,
  custom = false,
  onCustomChange,
}: DoubaoStreamEndpointSelectProps) {
  const selected = custom ? CUSTOM : doubaoStreamEndpointId(endpoint);
  return (
    <ControlledSelect
      aria-label="流式接口"
      value={selected}
      onChange={(value) => {
        onCustomChange?.(value === CUSTOM);
        updateDoubaoStreamEndpoint(value, onChange);
      }}
    >
      {doubaoStreamEndpointOptions()}
    </ControlledSelect>
  );
}

/**
 * 豆包的识别接口只用一个控件选：下拉里是两个流式预设，末项「自定义地址」；选了它，或者存的地址不是预设，下面多一行地址输入框。两行写的本来就是同一个 `asr_endpoint`，以前「流式接口」下拉和「识别接口地址」输入框各占一行。
 *
 * 地址为空时识别请求用豆包的默认地址（双向流式），所以下拉照实显示它，而不是显示成自定义。在输入框里打字时即使打出预设的地址，输入框也不收起，免得打到一半控件消失。
 */
export function DoubaoStreamEndpointSection({
  endpoint,
  onChange,
}: DoubaoStreamEndpointSectionProps) {
  const presetId = doubaoStreamEndpointId(endpoint.trim() || ASR_PROVIDER_DEFAULTS.doubao.endpoint);
  const { custom, setCustom } = useCustomEndpointIntent({ baseCustom: presetId === CUSTOM });
  return (
    <>
      <SelectRow
        title="流式接口"
        description="整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，边说边显示刷新更频繁。"
        aria-label="流式接口"
        value={custom ? CUSTOM : presetId}
        onChange={(event) => {
          const value = event.target.value;
          setCustom(value === CUSTOM);
          updateDoubaoStreamEndpoint(value, onChange);
        }}
      >
        {doubaoStreamEndpointOptions()}
      </SelectRow>
      {custom && (
        <EndpointSettingRow
          title="自定义接口地址"
          description="留空使用豆包的默认地址"
          inputLabel="识别接口地址"
          value={endpoint}
          onChange={(value) => {
            setCustom(true);
            onChange(value);
          }}
        />
      )}
    </>
  );
}
