import { useEffect, useState } from "react";
import { StatusMessage } from "../core/status-message";
import { useAsyncActionRunner } from "../core/use-async-action";
import { ActionButton } from "../core/action-button";
import { SettingActionHeader } from "../settings/setting-action-header";

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
  const [notice, setNotice] = useState("点击刷新读取可用录音设备");
  const { busy, run } = useAsyncActionRunner(
    (message) => {
      if (message) setNotice(message);
    },
    undefined,
    read,
  );
  useEffect(() => {
    setDevices([]);
  }, [read]);
  async function refresh() {
    void run(
      async (isCurrent) => {
        const result = await read();
        if (!isCurrent()) return;
        setDevices(result);
        setNotice(
          result.length
            ? "选择设备后从下一次录音生效"
            : "未发现设备，可手动填写设备名称或使用默认设备",
        );
      },
      { formatError: () => "无法读取设备列表，可重试或手动填写" },
    );
  }
  const selected = devices.findIndex((item) => item.backend === backend && item.id === device);
  return (
    <div>
      <SettingActionHeader as="label" title="可用录音设备">
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
        <ActionButton
          action={() => void refresh()}
          ariaBusy={busy}
          className=""
          disabled={busy}
          label={busy ? "读取中…" : "刷新设备"}
        />
      </SettingActionHeader>
      <StatusMessage role="status">{notice}</StatusMessage>
    </div>
  );
}
