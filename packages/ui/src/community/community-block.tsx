import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { ActionButton } from "../core/action-button";
import { GroupList, Row } from "../core/platform-controls";
import { useToast } from "../core/toast";
import { useMountedRef } from "../settings/use-mounted-ref";
import { errorCode } from "../core/error-code";
import { CommunityConfirmationActions } from "./community-confirmation-actions";
import { CommunityErrorAlert } from "./community-error-alert";
import { communityBlockMessage } from "./community-helpers";
import {
  communityBlockConfirmMessage,
  communityBlockConfirmTitle,
  communityBlockDay,
  communityBlockedAuthorsEmpty,
  communityBlockedAuthorsLoadError,
  communityBlockedNotice,
  communityBlockSignInMessage,
  type CommunityBlock,
  type CommunityBlockClient,
  type CommunityBlockKind,
} from "./community-block-model";
import * as style from "./community-style";

/** 设置外壳交给社区页面的屏蔽作者能力：宿主的屏蔽客户端、问有没有登录水杉账号的办法和去登录的入口。宿主不提供屏蔽客户端或账号客户端时没有这个值，页面就不画「屏蔽作者」和「已屏蔽的作者」。 */
export interface CommunityBlocksValue {
  client: CommunityBlockClient;
  /** 有没有登录的水杉账号；屏蔽只认登录账号，本机匿名账号不算。 */
  signedIn(): Promise<boolean>;
  onLogin?: () => void;
  /** 触屏宿主：「已屏蔽的作者」在「我的 › 隐私」里；否则在「关于 › 许可与隐私」里。确认屏蔽的说明据此指路。 */
  mobile: boolean;
}

export const CommunityBlocksContext = createContext<CommunityBlocksValue | null>(null);

export function useCommunityBlocks(): CommunityBlocksValue | null {
  return useContext(CommunityBlocksContext);
}

/**
 * 设置外壳用宿主的屏蔽客户端和账号客户端拼出 `CommunityBlocksContext` 的值；两者缺一就没有值。登录状态问账号客户端的 `status`：有 `user` 就是登录了，本机匿名账号不在那里。值只随两个客户端和宿主是不是触屏（宿主不会中途改变）变化，`onLogin` 每次渲染换一个也不会让名单重读。
 */
export function useCommunityBlocksValue(
  client: CommunityBlockClient | undefined,
  account: { status(): Promise<{ user?: unknown }> } | undefined,
  onLogin: () => void,
  mobile: boolean,
): CommunityBlocksValue | null {
  const login = useRef(onLogin);
  useLayoutEffect(() => {
    login.current = onLogin;
  });
  return useMemo(
    () =>
      client && account
        ? {
            client,
            signedIn: async () => Boolean((await account.status()).user),
            onLogin: () => login.current(),
            mobile,
          }
        : null,
    [client, account, mobile],
  );
}

export interface CommunityBlockAuthorSectionProps {
  kind: CommunityBlockKind;
  /** 作品的 id；服务端按它找到作者。 */
  id: string;
  author: string;
  /** 详情里别的操作正在进行。 */
  actionBusy: boolean;
  /** 屏蔽成功后以「已屏蔽「作者」」调用：图库据此关掉详情、显示提示并重读列表，这位作者的作品随之消失。 */
  onBlocked: (notice: string) => void;
}

/**
 * 别人作品详情里的「屏蔽作者」，与 iOS 的 `CommunityBlockAuthorButton`、Android 的 `CommunityBlockDialog` 同一套流程和措辞：没有登录时提示先登录；登录了先确认，确认后屏蔽。屏蔽不公开任何内容，所以不受社区投稿类功能的总闸（`HostCapabilities.community_contribution`）限制。没有 `CommunityBlocksContext` 时什么也不画。
 */
export function CommunityBlockAuthorSection({
  kind,
  id,
  author,
  actionBusy,
  onBlocked,
}: CommunityBlockAuthorSectionProps) {
  const blocks = useCommunityBlocks();
  const toast = useToast();
  const mounted = useMountedRef();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  if (!blocks) return null;
  const fail = (failure: unknown) => {
    const message = communityBlockMessage(failure, true);
    if (message === null) return;
    setError(message);
    setSignInRequired(errorCode(failure) === "community_unauthorized");
  };
  const start = async () => {
    setError("");
    setSignInRequired(false);
    setBusy(true);
    try {
      const signedIn = await blocks.signedIn();
      if (!mounted.current) return;
      if (signedIn) setConfirming(true);
      else {
        setError(communityBlockSignInMessage);
        setSignInRequired(true);
      }
    } catch (failure) {
      if (mounted.current) fail(failure);
    } finally {
      if (mounted.current) setBusy(false);
    }
  };
  const block = async () => {
    setBusy(true);
    try {
      const blocked = await blocks.client.block(kind, id);
      if (!mounted.current) return;
      setConfirming(false);
      const notice = communityBlockedNotice(blocked.name || author);
      toast(notice);
      onBlocked(notice);
    } catch (failure) {
      if (!mounted.current) return;
      setConfirming(false);
      fail(failure);
    } finally {
      if (mounted.current) setBusy(false);
    }
  };
  const title = communityBlockConfirmTitle(author);
  return (
    <>
      {error && (
        <CommunityErrorAlert
          message={error}
          signInRequired={signInRequired}
          onLogin={blocks.onLogin}
        />
      )}
      {confirming ? (
        <div className={style.confirmation} role="alertdialog" aria-label={title}>
          <p className="font-semibold">{title}</p>
          <p>{communityBlockConfirmMessage(blocks.mobile)}</p>
          <CommunityConfirmationActions>
            <ActionButton
              action={block}
              className="danger"
              disabled={busy}
              ariaBusy={busy}
              label="屏蔽"
            />
            <ActionButton
              action={() => setConfirming(false)}
              disabled={busy}
              ariaBusy={busy}
              label="取消"
            />
          </CommunityConfirmationActions>
        </div>
      ) : (
        <ActionButton
          action={start}
          className="danger-text self-start pt-0"
          disabled={actionBusy || busy}
          label="屏蔽作者"
        />
      )}
    </>
  );
}

/**
 * 「已屏蔽的作者」页的内容：登录账号屏蔽的作者，最近的在前，每行是作者名、屏蔽日期和「取消屏蔽」。取消后这一行去掉，社区列表下次读取时这位作者的作品就回来了。读不到名单时说明并给「重试」；登录失效时请用户登录。
 */
export function CommunityBlockedAuthors({ blocks }: { blocks: CommunityBlocksValue }) {
  const mounted = useMountedRef();
  const [items, setItems] = useState<CommunityBlock[] | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [signInRequired, setSignInRequired] = useState(false);
  const [removing, setRemoving] = useState<ReadonlySet<string>>(() => new Set());
  const [error, setError] = useState("");
  const load = useCallback(async () => {
    setLoadFailed(false);
    setSignInRequired(false);
    try {
      const value = await blocks.client.list();
      if (mounted.current) setItems(value);
    } catch (failure) {
      if (!mounted.current || errorCode(failure) === "community_cancelled") return;
      setSignInRequired(errorCode(failure) === "community_unauthorized");
      setLoadFailed(true);
    }
  }, [blocks.client, mounted]);
  useEffect(() => {
    void load();
  }, [load]);
  const unblock = async (item: CommunityBlock) => {
    setError("");
    setSignInRequired(false);
    setRemoving((current) => new Set(current).add(item.id));
    try {
      await blocks.client.unblock(item.id);
      if (!mounted.current) return;
      setItems((current) => current?.filter((block) => block.id !== item.id) ?? current);
    } catch (failure) {
      if (!mounted.current) return;
      const message = communityBlockMessage(failure);
      if (message !== null) setError(message);
      setSignInRequired(errorCode(failure) === "community_unauthorized");
    } finally {
      if (mounted.current)
        setRemoving((current) => {
          const next = new Set(current);
          next.delete(item.id);
          return next;
        });
    }
  };
  // 读过的名单留着：只有从没读到过时，失败才换掉整页。
  if (items === null && loadFailed) {
    return signInRequired ? (
      <CommunityErrorAlert
        message={communityBlockSignInMessage}
        signInRequired
        onLogin={blocks.onLogin}
      />
    ) : (
      <>
        <CommunityErrorAlert message={communityBlockedAuthorsLoadError} />
        <ActionButton action={load} className="secondary self-start" label="重试" />
      </>
    );
  }
  if (items === null) return <p className={style.notice}>正在读取…</p>;
  if (items.length === 0) return <p className={style.notice}>{communityBlockedAuthorsEmpty}</p>;
  return (
    <>
      {error && (
        <CommunityErrorAlert
          message={error}
          signInRequired={signInRequired}
          onLogin={blocks.onLogin}
        />
      )}
      <GroupList>
        {items.map((item) => (
          <Row key={item.id} title={item.name} detail={communityBlockDay(item.created_at)}>
            <ActionButton
              action={() => unblock(item)}
              disabled={removing.has(item.id)}
              ariaBusy={removing.has(item.id)}
              ariaLabel={`取消屏蔽 ${item.name}`}
              label="取消屏蔽"
            />
          </Row>
        ))}
      </GroupList>
    </>
  );
}
