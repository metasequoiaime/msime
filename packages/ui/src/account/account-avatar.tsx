import { useEffect, useState } from "react";
import type { AccountUser } from "./account-model";
import * as account from "./account-style";
import { useAsyncGeneration } from "../settings/use-async-generation";

/** Images already fetched, by avatar URL, so the profile card and the edit dialog showing the same avatar ask the host once. A failed load is dropped, so the next mount tries again. */
const AVATAR_CACHE_CAPACITY = 16;
const loaded = new Map<string, Promise<string | null>>();

function loadAvatar(url: string, load: () => Promise<string | null>): Promise<string | null> {
  const cached = loaded.get(url);
  if (cached) {
    loaded.delete(url);
    loaded.set(url, cached);
    return cached;
  }
  const pending = load().catch(() => {
    if (loaded.get(url) === pending) loaded.delete(url);
    return null;
  });
  loaded.set(url, pending);
  while (loaded.size > AVATAR_CACHE_CAPACITY) {
    const oldest = loaded.keys().next().value;
    if (oldest === undefined) break;
    loaded.delete(oldest);
  }
  return pending;
}

/**
 * 没有头像或尚未加载时，显示共享显示名的首个 Unicode 码点。
 *
 * 页面不加载远程图片；宿主取回图片并提供 `data:` URL，`user.avatarUrl` 变化时重新请求。未提供 `load` 时只显示首字。
 */
export function AccountAvatar({
  user,
  name,
  load,
  size,
}: {
  user: AccountUser;
  /** 编辑页传入已规范化的草稿，覆盖已保存的显示名。 */
  name?: string;
  load?: () => Promise<string | null>;
  size: account.AvatarSize;
}) {
  const url = user.avatarUrl;
  const [image, setImage] = useState<{ url: string; src: string } | null>(null);
  const generation = useAsyncGeneration(url, load);
  useEffect(() => {
    if (!url || !load) return;
    const requestGeneration = generation.current;
    void loadAvatar(url, load).then((src) => {
      if (generation.current === requestGeneration && src) setImage({ url, src });
    });
  }, [url, load, generation]);
  const src = image && image.url === url ? image.src : null;
  return (
    <div className={`${account.avatar(size)} overflow-hidden`} aria-hidden="true">
      {src ? (
        <img className="size-full object-cover" src={src} alt="" draggable={false} />
      ) : (
        Array.from(name || user.preferredName)[0]
      )}
    </div>
  );
}
