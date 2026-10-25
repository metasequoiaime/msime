import { useEffect, useState } from "react";
import { ActionButton } from "../core/action-button";
import { errorCode } from "../core/error-code";
import { formatDateTime } from "../core/format-date";
import { StatusMessage } from "../core/status-message";
import { accountMessage } from "./account-errors";
import type { AccountQqLink, QqLinksClient } from "./account-model";
import { useAccountAction } from "./account-operation";
import * as account from "./account-style";
import { MeSubpageHeader } from "./mobile-download-page";

/** 绑定失败时的说法；其余错误沿用账号页的通用说法。 */
export function qqLinkMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "account_invalid":
      return "绑定码是 8 位数字，请检查后重试。";
    case "account_qq_link_code_not_found":
      return "绑定码不对或已过期（10 分钟内有效，只能用一次）。请在 QQ 群里重新发 /bind 拿一个新的。";
    case "account_qq_link_limit":
      return "这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。";
    case "account_rate_limited":
      return "尝试次数太多，请一小时后再试。";
  }
  return accountMessage(error);
}

/**
 * 账号页的「绑定 QQ 群助手」：输入机器人给的绑定码，列出已有的绑定，可以解绑。绑定后在 QQ 群里 @机器人 发 /stats 能看到打字数据，数据来自打字统计页的「上传到云端」。
 */
export function QqLinkCard({
  client,
  userId,
  showHeading = true,
}: {
  client: QqLinksClient;
  userId: string;
  showHeading?: boolean;
}) {
  const [links, setLinks] = useState<AccountQqLink[] | null>(null);
  const [code, setCode] = useState("");
  const [adding, setAdding] = useState(false);
  const [message, setMessage] = useState("");
  const { busy, mounted, clientGeneration, perform } = useAccountAction(
    client,
    setMessage,
    setMessage,
    userId,
  );

  const reload = async () => {
    const current = clientGeneration.current;
    const next = await client.list();
    if (mounted.current && clientGeneration.current === current) setLinks(next);
  };

  useEffect(() => {
    setLinks(null);
    setCode("");
    setAdding(false);
    setMessage("");
    void perform(reload, { allowBusy: true });
  }, [client, userId]);

  const digits = code.replace(/[\s-]/g, "");
  const redeem = () => {
    if (busy) return;
    setMessage("");
    void perform(async () => {
      try {
        await client.redeem(digits);
      } catch (error) {
        // 绑定码的几种失败各有说法，不交给通用的账号错误。
        if (mounted.current) setMessage(qqLinkMessage(error));
        return;
      }
      if (!mounted.current) return;
      setCode("");
      await reload();
      if (mounted.current) {
        setAdding(false);
        setMessage("绑定成功。");
      }
    });
  };
  const hasLinks = Boolean(links?.length);
  const showForm = links !== null && (!hasLinks || adding);

  return (
    <section className={`${account.section} ${account.stack}`}>
      {showHeading && (
        <h2 className={account.heading}>{hasLinks ? "QQ 群助手" : "绑定 QQ 群助手"}</h2>
      )}
      {hasLinks && (
        <p className={account.note}>
          已绑定。在 QQ 群里 @机器人 发 /stats
          就能看到你的打字数据（先在「打字统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。
        </p>
      )}
      {showForm && (
        <>
          <p className={account.note}>
            在水杉输入法的 QQ 群里 @机器人 发 /bind，把收到的 8 位绑定码填在这里。绑定后发 /stats
            能看到你的打字数据（先在「打字统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。
          </p>
          <label className={account.field}>
            绑定码
            <input
              className={account.input}
              inputMode="numeric"
              autoComplete="one-time-code"
              maxLength={10}
              placeholder="8 位数字"
              value={code}
              disabled={busy}
              onChange={(event) => setCode(event.target.value)}
            />
          </label>
          <div className={account.actionRow}>
            <ActionButton
              action={redeem}
              ariaBusy={busy}
              className={account.primary}
              disabled={busy || !/^[0-9]{8}$/.test(digits)}
              label="绑定"
            />
            {hasLinks && (
              <button
                type="button"
                className="secondary"
                disabled={busy}
                onClick={() => {
                  setAdding(false);
                  setCode("");
                  setMessage("");
                }}
              >
                取消
              </button>
            )}
          </div>
        </>
      )}
      {hasLinks && (
        <ul className="m-0 flex list-none flex-col gap-2 p-0" aria-label="已绑定的 QQ 群友">
          {links?.map((link) => (
            <li key={link.id} className="flex items-center justify-between gap-3">
              <span className={account.muted}>绑定于 {formatDateTime(link.createdAt)}</span>
              <ActionButton
                action={() =>
                  void perform(async () => {
                    await client.remove(link.id);
                    await reload();
                    if (mounted.current) setMessage("已解绑。");
                  })
                }
                className="secondary"
                disabled={busy}
                label="解绑"
              />
            </li>
          ))}
        </ul>
      )}
      {hasLinks && !adding && (
        <div className={account.actionRow}>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => {
              setAdding(true);
              setMessage("");
            }}
          >
            绑定其他 QQ
          </button>
        </div>
      )}
      {links === null && !busy && message && (
        <ActionButton action={() => perform(reload)} className="secondary" label="重试" />
      )}
      {busy && <StatusMessage role="status">正在处理…</StatusMessage>}
      {message && <StatusMessage role="status">{message}</StatusMessage>}
    </section>
  );
}

/** 触屏「我的 → 同步 → 绑定 QQ 群助手」二级页，与 Android 的独立页面保持同一导航层级。 */
export function MobileQqLinksPage({
  client,
  userId,
  onBack,
}: {
  client: QqLinksClient;
  userId: string;
  onBack: () => void;
}) {
  return (
    <div className={account.subpage}>
      <MeSubpageHeader title="绑定 QQ 群助手" onBack={onBack} />
      <QqLinkCard client={client} userId={userId} showHeading={false} />
    </div>
  );
}
