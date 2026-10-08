import { useEffect, useState } from "react";
import type { AccountUser } from "./account-page";
import { preferredAccountName } from "./account-labels";
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
 * The user's avatar, or the first character of their name while there is none or it has not loaded.
 *
 * The page loads no remote image, so the host fetches the avatar and hands it over as a `data:` URL; `user.avatarUrl` changes whenever the avatar does, which is what decides when to ask again. Without a `load` (the mobile hosts) the initial is all there is.
 */
export function AccountAvatar({
  user,
  name,
  load,
  size,
}: {
  user: AccountUser;
  /** The name whose first character stands in for a missing avatar; the edit dialog passes the name being typed. */
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
        (name?.trim() || preferredAccountName(user)).slice(0, 1)
      )}
    </div>
  );
}
