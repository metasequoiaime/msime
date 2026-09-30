import type { Preferences } from "../index";
import { validCandidateFonts } from "../candidate/candidate-font-family";
import { SettingsActionsFooter, type SettingsActionsFooterProps } from "./settings-actions-footer";

export type SettingsFormFooterProps = Omit<SettingsActionsFooterProps, "canSave"> & {
  draft: Preferences;
};

/** Combines candidate-font validation feedback with the shared settings actions. */
export function SettingsFormFooter({ draft, ...footerProps }: SettingsFormFooterProps) {
  const canSave = validCandidateFonts(draft);

  return (
    <>
      {!canSave && (
        <p role="alert">
          请在候选窗口页修正字体：名称不能为空、不能含控制字符或超过 128 个 UTF-8 字节，补充字体最多 32
          项。
        </p>
      )}
      <SettingsActionsFooter {...footerProps} canSave={canSave} />
    </>
  );
}
