import type { Preferences } from "../index";
import { validCandidateFonts } from "../candidate/candidate-font-family";
import { SettingsActionsFooter, type SettingsActionsFooterProps } from "./settings-actions-footer";

export type SettingsFormFooterProps = SettingsActionsFooterProps & {
  draft: Preferences;
};

/** Combines candidate-font validation feedback, which holds back the automatic save, with the shared settings actions. */
export function SettingsFormFooter({ draft, ...footerProps }: SettingsFormFooterProps) {
  const canSave = validCandidateFonts(draft);

  // The form's pages end with their last group, so the action row keeps the groups' own spacing from it.
  return (
    <div className="mt-6">
      {!canSave && (
        <p role="alert">
          请在候选窗口页修正字体：名称不能为空、不能含控制字符或超过 128 个 UTF-8 字节。
        </p>
      )}
      <SettingsActionsFooter {...footerProps} />
    </div>
  );
}
