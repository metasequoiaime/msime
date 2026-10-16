export type VoiceCaptureDevice = {
  backend: "pulse" | "pipewire" | "alsa" | "windows" | "macos" | "harmony";
  id: string;
  label: string;
};
export type VoiceDeviceReader = () => Promise<VoiceCaptureDevice[]>;

/** 麦克风下拉里「系统默认设备」的值，也是存储里的空设备。 */
export const SYSTEM_DEFAULT = "";
/** 存着的设备不在读到的列表里（拔掉了，或者还没读完）时，为它保留的那一项的值。 */
export const STORED = "stored";
/** 存着别的平台的后端时显示的那一项的值。它和「系统默认设备」分开，用户选「系统默认设备」才是一次真正的改动，能把外来的后端和设备清掉。 */
export const FOREIGN = "foreign";
