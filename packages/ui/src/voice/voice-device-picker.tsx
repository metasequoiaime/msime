import { useEffect, useRef, useState } from "react";

export type VoiceCaptureDevice = {
  backend: "pulse" | "pipewire" | "alsa" | "windows" | "macos" | "harmony";
  id: string;
  label: string;
};
export type VoiceDeviceReader = () => Promise<VoiceCaptureDevice[]>;

export function VoiceDevicePicker({
  read,
  backend,
  device,
  choose,
}: {
  read: VoiceDeviceReader;
  backend: string;
  device: string;
  choose: (backend: VoiceCaptureDevice["backend"], device: string) => void;
}) {
  const [devices, setDevices] = useState<VoiceCaptureDevice[]>([]);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("点击刷新读取可用录音设备");
  const pending = useRef(false);
  const revision = useRef(0);
  useEffect(() => {
    pending.current = false;
    setBusy(false);
    setDevices([]);
    return () => {
      revision.current++;
    };
  }, [read]);
  async function refresh() {
    if (pending.current) return;
    const current = ++revision.current;
    pending.current = true;
    setBusy(true);
    try {
      const result = await read();
      if (current !== revision.current) return;
      setDevices(result);
      setNotice(
        result.length
          ? "选择设备后从下一次录音生效"
          : "未发现设备，可手动填写设备名称或使用默认设备",
      );
    } catch {
      if (current === revision.current) setNotice("无法读取设备列表，可重试或手动填写");
    } finally {
      if (current === revision.current) {
        pending.current = false;
        setBusy(false);
      }
    }
  }
  const selected = devices.findIndex((item) => item.backend === backend && item.id === device);
  return (
    <div>
      <label className="section-header">
        <span className="section-title">可用录音设备</span>
        <select
          aria-label="可用录音设备"
          value={selected < 0 ? "" : String(selected)}
          disabled={!devices.length || busy}
          onChange={(event) => {
            const item = devices[Number(event.target.value)];
            if (item) choose(item.backend, item.id);
          }}
        >
          <option value="" disabled>
            选择设备
          </option>
          {devices.map((item, index) => (
            <option key={`${item.backend}:${item.id}`} value={index}>
              {item.label} ({item.backend})
            </option>
          ))}
        </select>
        <button type="button" disabled={busy} onClick={() => void refresh()}>
          {busy ? "读取中…" : "刷新设备"}
        </button>
      </label>
      <p role="status">{notice}</p>
    </div>
  );
}
