import { useEffect, useState, type MutableRefObject } from "react";
import type { AccountChallenge, AccountClient } from "./account-page";

export type AccountChannel = "email" | "phone";

/** Verification-code state shared by the account page and the mobile login sheet. */
export function useAccountVerification({
  client,
  mounted,
  clientGeneration,
  setNotice,
}: {
  client: Pick<AccountClient, "requestCode" | "login">;
  mounted: MutableRefObject<boolean>;
  clientGeneration: MutableRefObject<number>;
  setNotice: (message: string) => void;
}) {
  const [channel, setChannel] = useState<AccountChannel | null>(null);
  const [target, setTarget] = useState("");
  const [code, setCode] = useState("");
  const [challenge, setChallenge] = useState<AccountChallenge | null>(null);
  const [expiresAt, setExpiresAt] = useState(0);
  const [resendAt, setResendAt] = useState(0);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!challenge) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [challenge]);

  const selectChannel = (value: AccountChannel) => {
    setChannel(value);
    setTarget("");
    setCode("");
    setChallenge(null);
  };

  const updateTarget = (value: string) => {
    setTarget(value);
    setChallenge(null);
    setCode("");
  };

  const sendCode = async () => {
    const generation = clientGeneration.current;
    if (!channel) return;
    const normalized = target.trim();
    if (!normalized) throw { code: "account_invalid" };
    const value = await client.requestCode(channel, normalized);
    if (!mounted.current || generation !== clientGeneration.current) return;
    const timestamp = Date.now();
    setNow(timestamp);
    setChallenge(value);
    setExpiresAt(timestamp + value.expiresIn * 1000);
    setResendAt(timestamp + 60_000);
    setCode("");
    setNotice("验证码已发送，请查收。");
  };

  const signIn = async () => {
    const generation = clientGeneration.current;
    if (!challenge || !/^\d{6}$/.test(code) || expiresAt <= Date.now())
      throw { code: "account_invalid" };
    const result = await client.login(challenge.challengeId, code);
    if (!mounted.current || generation !== clientGeneration.current) return undefined;
    if (!result.user) throw { code: "account_unavailable" };
    return { user: result.user, generation };
  };

  return {
    channel,
    setChannel,
    target,
    code,
    setCode,
    challenge,
    setChallenge,
    expiresAt,
    resendSeconds: Math.max(0, Math.ceil((resendAt - now) / 1000)),
    expired: Boolean(challenge) && expiresAt <= now,
    selectChannel,
    updateTarget,
    sendCode,
    signIn,
  };
}
