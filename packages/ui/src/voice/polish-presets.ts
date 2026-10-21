/** 内置名称与正文读取共享 Rust 导出，公开 id 元组由同一个目录生成。 */
import catalog from "./polish-prompt-catalog.json";
import { POLISH_PRESET_IDS } from "./polish-preset-ids";

export { POLISH_PRESET_IDS } from "./polish-preset-ids";
export type PolishPresetId = (typeof POLISH_PRESET_IDS)[number];
export const POLISH_CUSTOM_IDS = ["custom_1", "custom_2", "custom_3"] as const;

export const POLISH_PRESET_NAMES = Object.fromEntries(
  catalog.map(({ id, title }) => [id, title]),
) as Record<PolishPresetId, string>;

export const POLISH_PRESETS = Object.fromEntries(
  catalog.map(({ id, prompt }) => [id, prompt]),
) as Record<PolishPresetId, string>;

/** The prompt behind a preset id, or "" when the id names a custom slot. */
export function polishPresetPrompt(id: string | undefined): string {
  return (POLISH_PRESETS as Record<string, string>)[id ?? ""] ?? "";
}
export function isPolishCustomSlot(id: string | undefined): boolean {
  return (POLISH_CUSTOM_IDS as readonly string[]).includes(id ?? "");
}

/** Returns the preferences field used to store a custom prompt slot. */
export function polishSlotField(slot: string): string | undefined {
  return isPolishCustomSlot(slot) ? `polish_prompt_${slot}` : undefined;
}

/** Resolves the prompt text shown for a preset or a stored custom slot. */
export function polishPromptFor<T extends object>(slot: string, current: T): string {
  const field = polishSlotField(slot);
  if (!field) return polishPresetPrompt(slot);
  return ((current as Record<string, unknown>)[field] as string) ?? "";
}
