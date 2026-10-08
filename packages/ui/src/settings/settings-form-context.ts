import { createContext, useContext } from "react";
import type { Preferences, SettingsPageModel } from "../index";

/**
 * What a page of the settings form reads. The form is only drawn once the preferences have loaded, so its pages see the draft as present rather than each re-checking it.
 */
export type SettingsFormModel = Omit<SettingsPageModel, "draft"> & { draft: Preferences };

export const SettingsFormContext = createContext<SettingsFormModel | null>(null);

export function useSettingsForm(): SettingsFormModel {
  const model = useContext(SettingsFormContext);
  if (!model) throw new Error("A settings page was rendered outside SettingsPage's form.");
  return model;
}

/** 调用方位于 `SettingsPage` 的表单内时返回表单模型，否则为 null；供那些也会在表单外绘制的基础组件（测试、引导流程、独立预览）使用，只在有表单时才适配。 */
export function useOptionalSettingsForm(): SettingsFormModel | null {
  return useContext(SettingsFormContext);
}
