import {
  DOUBAO_STREAM_ENDPOINTS,
  doubaoStreamEndpointId,
  findDoubaoStreamEndpoint,
} from "../voice/voice-providers";
import { SelectRow } from "./select-row";

export interface DoubaoStreamEndpointSectionProps {
  endpoint: string;
  onChange: (endpoint: string) => void;
}

export interface DoubaoStreamEndpointSelectProps {
  endpoint: string;
  onChange: (endpoint: string) => void;
}

/** 一个返回 Fragment 的普通函数而不是组件：`SelectRow` 在 HarmonyOS 上只认直接的 `<option>` 和 Fragment 子元素。 */
function doubaoStreamEndpointOptions() {
  return (
    <>
      {DOUBAO_STREAM_ENDPOINTS.map((option) => (
        <option key={option.id} value={option.id}>
          {option.title}
        </option>
      ))}
      <option value="custom">自定义地址</option>
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
}: DoubaoStreamEndpointSelectProps) {
  const selected = doubaoStreamEndpointId(endpoint);
  return (
    <select
      aria-label="流式接口"
      value={selected}
      onChange={(event) => updateDoubaoStreamEndpoint(event.target.value, onChange)}
    >
      {doubaoStreamEndpointOptions()}
    </select>
  );
}

/** Doubao streaming endpoint preset selector. */
export function DoubaoStreamEndpointSection({
  endpoint,
  onChange,
}: DoubaoStreamEndpointSectionProps) {
  return (
    <SelectRow
      title="流式接口"
      description="整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，流式预编辑刷新更频繁。选择后写入下方接口地址。"
      aria-label="流式接口"
      value={doubaoStreamEndpointId(endpoint)}
      onChange={(event) => updateDoubaoStreamEndpoint(event.target.value, onChange)}
    >
      {doubaoStreamEndpointOptions()}
    </SelectRow>
  );
}
