import { useCallback, useEffect, useRef, useState, type MutableRefObject } from "react";
import type { AccountClient, AccountUser } from "./account-page";

/** 账号页面共用的浏览器登录等待状态和迟到结果守卫。 */
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

  const resetWaiting = useCallback(() => {
    waitingRef.current = false;
    setWaiting(false);
  }, []);

  // 旧客户端的取消仍由消费者清理调用；新客户端提交后才清除等待标记。
  useEffect(resetWaiting, [client, resetWaiting]);

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

  return { waiting, cancel, signIn };
}
