import type { FeedbackType, ImeSetupState } from "../core/host-contracts";

/** 服务接受的报告正文长度上限，按字符计（Android `FeedbackApi.MAX_TEXT`）。 */
export const feedbackMaxText = 500;

/** 服务接受的报告类型及页面显示的名称（Android `FeedbackApi.Type`）。 */
export const feedbackTypes: readonly { value: FeedbackType; label: string }[] = [
  { value: "bug", label: "问题" },
  { value: "suggestion", label: "建议" },
  { value: "dictionary", label: "词库纠错" },
];

export interface FeedbackDiagnosticsSource {
  appVersion: string;
  edition?: string;
  host?: { platform: string; os_version?: string };
  preferences?: { scheme?: string; touch_keyboard_layout?: string; global_theme?: string };
  /** 输入法设置状态，宿主能判断时提供。 */
  setup?: ImeSetupState | null;
}

/** 生成反馈提交的诊断字段；缺少值的字段直接省略。 */
export function feedbackDiagnostics({
  appVersion,
  edition,
  host,
  preferences,
  setup,
}: FeedbackDiagnosticsSource): Record<string, string> {
  const fields: [string, string | undefined][] = [
    ["os", host?.os_version ? `HarmonyOS ${host.os_version}` : undefined],
    ["app_version", appVersion],
    ["edition", edition],
    ["scheme", preferences?.scheme],
    ["keyboard_layout", preferences?.touch_keyboard_layout],
    ["skin", preferences?.global_theme],
    ["ime_enabled", typeof setup?.enabled === "boolean" ? String(setup.enabled) : undefined],
    ["ime_default", typeof setup?.current === "boolean" ? String(setup.current) : undefined],
  ];
  const diagnostics: Record<string, string> = {};
  for (const [key, value] of fields) if (value) diagnostics[key] = value;
  return diagnostics;
}
