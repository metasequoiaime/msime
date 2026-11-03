import { useRef, useState, type ReactNode } from "react";
import { useConfirm } from "../core/confirm";
import { errorCode } from "../core/error-code";
import { SettingsExternalMeta } from "../settings/settings-external-meta";
import type { ExternalSkin, SkinCatalog } from "./external-skins";
import { notifySkinCatalogChanged } from "./skin-catalog-changes";

type Notice = { failed: boolean; text: string };

/**
 * 删除没成功时给用户的一句话，按宿主回的码。能不能删由宿主（client-services 的 `candidate_sync::delete_unselected`）决定，页面只把码翻成白话。
 *
 * `shownInUse`：卡片上正显示着「使用中」。槽位里留着这款皮肤、但当前全局主题不是自定义主题时，卡片的开关是关着的，要先打开再关掉才能把它从自定义主题里取下。
 */
export function skinDeletionFailure(
  code: string | undefined,
  name: string,
  shownInUse: boolean,
): string {
  switch (code) {
    case "candidate_skin_in_use":
      return shownInUse
        ? `「${name}」正在使用，先关掉它的开关再删除。`
        : `自定义主题里还留着「${name}」，切回自定义主题时会用上它。先打开再关掉它的开关，把它从自定义主题里取下，再删除。`;
    case "candidate_skin_not_installed":
      return `「${name}」已经不在皮肤目录里了。`;
    case "candidate_skin_package":
      return `「${name}」不是普通的皮肤文件夹，请打开皮肤目录手动删除。`;
    default:
      return `没能删除「${name}」，请稍后重试。`;
  }
}

/**
 * 「主题」页皮肤卡片上的「删除」：先确认，再交给宿主删，删完通知皮肤目录变了，轮播和社区页随之重扫。宿主没有 `deleteSkin` 时 `onDelete` 是 `undefined`，卡片上就没有这个按钮。
 *
 * `inUse` 是卡片上「使用中」的判断，只用来挑提示的说法，不用来拦删除。
 */
export function useSkinDeletion(
  deleteSkin: ((id: string) => Promise<SkinCatalog & { synced: boolean }>) | undefined,
  packages: readonly Pick<ExternalSkin, "id" | "name">[],
  inUse: (id: string) => boolean,
): { onDelete?: (id: string) => void; notice: ReactNode; confirmation: ReactNode } {
  const { confirm, confirmation } = useConfirm();
  const [notice, setNotice] = useState<Notice | null>(null);
  // 一次只删一款：确认框开着或宿主还没回时，再点「删除」不起作用。
  const pending = useRef(false);
  async function remove(id: string) {
    if (!deleteSkin || pending.current) return;
    pending.current = true;
    const name = packages.find((skin) => skin.id === id)?.name ?? id;
    try {
      setNotice(null);
      const confirmed = await confirm({
        title: "删除皮肤",
        message: `「${name}」的皮肤文件夹会从这台电脑上删掉，无法恢复。开着皮肤云端同步时，同步过的私人副本会在下次同步时一起从云端删掉；已经公开发布到社区的不受影响。`,
        confirmLabel: "删除",
        danger: true,
      });
      if (!confirmed) return;
      const result = await deleteSkin(id);
      setNotice({
        failed: false,
        text: result.synced
          ? `已删除「${name}」。云端皮肤库里的私人副本会在下次同步时一起删掉。`
          : `已删除「${name}」。`,
      });
      notifySkinCatalogChanged();
    } catch (error) {
      const code = errorCode(error);
      setNotice({ failed: true, text: skinDeletionFailure(code, name, inUse(id)) });
      // 已经不在目录里的皮肤，重扫一次让卡片也消失。
      if (code === "candidate_skin_not_installed") notifySkinCatalogChanged();
    } finally {
      pending.current = false;
    }
  }
  return {
    onDelete: deleteSkin ? (id) => void remove(id) : undefined,
    notice: notice && (
      <SettingsExternalMeta role={notice.failed ? "alert" : "status"}>
        {notice.text}
      </SettingsExternalMeta>
    ),
    confirmation,
  };
}
