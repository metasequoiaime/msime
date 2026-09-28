import { DOUBAO_STREAM_ENDPOINTS } from "../voice/voice-providers";

export interface DoubaoStreamEndpointSectionProps {
  endpoint: string;
  onChange: (endpoint: string) => void;
}

/** Doubao streaming endpoint preset selector. */
export function DoubaoStreamEndpointSection({
  endpoint,
  onChange,
}: DoubaoStreamEndpointSectionProps) {
  const selected =
    DOUBAO_STREAM_ENDPOINTS.find((option) => option.endpoint === endpoint)?.id ?? "custom";
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          流式接口
          <small>
            整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，流式预编辑刷新更频繁。选择后写入下方接口地址。
          </small>
        </span>
        <select
          aria-label="流式接口"
          value={selected}
          onChange={(event) => {
            const chosen = DOUBAO_STREAM_ENDPOINTS.find(
              (option) => option.id === event.target.value,
            );
            if (chosen) onChange(chosen.endpoint);
          }}
        >
          {DOUBAO_STREAM_ENDPOINTS.map((option) => (
            <option key={option.id} value={option.id}>
              {option.title}
            </option>
          ))}
          <option value="custom">自定义地址</option>
        </select>
      </label>
    </div>
  );
}
