import * as style from "./community-style";
import { ActionButton } from "../core/action-button";

/** Shared primary install action used by community detail pages. */
export function CommunityInstallButton({
  actionBusy,
  detailBusy,
  confirmReplace,
  onInstall,
}: {
  actionBusy: boolean;
  detailBusy: boolean;
  confirmReplace: boolean;
  onInstall: () => void;
}) {
  return (
    <ActionButton
      action={onInstall}
      className={`primary ${style.action}`}
      disabled={actionBusy || detailBusy || confirmReplace}
      ariaBusy={actionBusy}
      label={actionBusy ? "正在安装…" : "一键安装"}
    />
  );
}
