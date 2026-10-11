import { useRef, useState, type MutableRefObject } from "react";
import type { AccountClient, AccountUser } from "./account-page";

/** Shared browser-login waiting state and stale-result guard for account surfaces. */
export function useAccountGoogleLogin({
  client,
  mounted,
  clientGeneration,
  cancelOnce = false,
}: {
  client: Pick<AccountClient, "googleLogin" | "googleCancel">;
  mounted: MutableRefObject<boolean>;
  clientGeneration: MutableRefObject<number>;
  /** A closing sheet may request cancellation again during unmount. */
  cancelOnce?: boolean;
}) {
  const [waiting, setWaiting] = useState(false);
  const waitingRef = useRef(false);

  const resetWaiting = () => {
    waitingRef.current = false;
    setWaiting(false);
  };

  const cancel = () => {
    if (!waitingRef.current || !client.googleCancel) return;
    if (cancelOnce) waitingRef.current = false;
    // The pending googleLogin reports the outcome; cancellation failure means no request remains.
    void client.googleCancel().catch(() => undefined);
  };

  const signIn = async () => {
    const generation = clientGeneration.current;
    if (!client.googleLogin) throw { code: "account_unavailable" };
    waitingRef.current = true;
    setWaiting(true);
    let result: { user?: AccountUser | null };
    try {
      result = await client.googleLogin();
    } finally {
      if (mounted.current && generation === clientGeneration.current) resetWaiting();
    }
    if (!mounted.current || generation !== clientGeneration.current) return undefined;
    if (!result.user) throw { code: "account_unavailable" };
    return { user: result.user, generation };
  };

  return { waiting, resetWaiting, cancel, signIn };
}
