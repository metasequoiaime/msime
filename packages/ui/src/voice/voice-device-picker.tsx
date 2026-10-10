import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { ActionRow } from "../settings/action-row";
import { SelectRow } from "../settings/select-row";

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
  // 「录音设备」分组里的两行：先读取设备列表，再从中选择。HarmonyOS 手机上选择行与其他选择行一样点开选择面板。
  return (
    <>
      <ActionRow
        title="读取录音设备"
        description={<span role="status">{notice}</span>}
        action={() => void refresh()}
        ariaBusy={busy}
        className="secondary m-0"
        disabled={busy}
        label={busy ? "读取中…" : "刷新设备"}
      />
      <SelectRow
        title="可用录音设备"
        aria-label="可用录音设备"
        value={selected < 0 ? "" : String(selected)}
        disabled={!devices.length || busy}
        onChange={(event) => {
          const item = devices[Number(event.target.value)];
          if (item) choose(item.backend, item.id);
        }}
      >
        <option value="" disabled hidden>
          选择设备
        </option>
        {devices.map((item, index) => (
          <option key={`${item.backend}:${item.id}`} value={index}>
            {item.label} ({item.backend})
          </option>
        ))}
      </SelectRow>
    </>
  );
}
