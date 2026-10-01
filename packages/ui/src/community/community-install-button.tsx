import * as style from "./community-style";

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
    <button
      type="button"
      className={`primary ${style.action}`}
      disabled={actionBusy || detailBusy || confirmReplace}
      onClick={onInstall}
    >
      {actionBusy ? "正在安装…" : "一键安装"}
    </button>
  );
}
