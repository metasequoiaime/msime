import type { ReactNode } from "react";
import { SelectRow } from "./select-row";
import { VoiceProviderOptions, type VoiceProviderOption } from "../voice/voice-provider-options";

export interface VoiceProviderRowProps {
  title: ReactNode;
  description?: ReactNode;
  options: readonly VoiceProviderOption[];
  ariaLabel: string;
  value: string;
  disabled?: boolean;
  children?: ReactNode;
  onChange: (provider: string) => void;
}

/** 语音功能共用的服务选择行。HarmonyOS 手机上整行点开选择面板，与其他 `SelectRow` 一致。 */
export function VoiceProviderRow({
  title,
  description,
  options,
  ariaLabel,
  value,
  disabled,
  children,
  onChange,
}: VoiceProviderRowProps) {
  return (
    <SelectRow
      title={title}
      description={description}
      aria-label={ariaLabel}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
    >
      {/* 直接调用而不是作为组件渲染：HarmonyOS 的选择面板只读取直接的 `<option>` 子元素。 */}
      {VoiceProviderOptions({ options })}
      {children}
    </SelectRow>
  );
}
