/**
 * 设置应用和键盘两个沙箱之间的会话镜像（#8201 第 2 步，迁移差异 M2、M5）：各自的 `files/state` 里是各自的会话文件，另一边经共享沙箱发布的那一份由 Rust 的 `session_adopt` 在本沙箱的 `account-refresh.lock` 里按 `user.id` 和 `expires_at` 决定要不要采用，宿主不再整份覆盖。
 *
 * 顺序是这里的要点：服务端每次刷新都轮换刷新令牌，拿用过的那枚再刷新超过 30 秒宽限就是重放，整个会话族被撤销。所以每个沙箱在发任何认证请求之前，都要先采用另一边发布的会话；`AdoptionGate` 把采用排成一条队，每个认证请求自己排一次采用、等它做完再发。
 *
 * 这里只有规则：拼请求、读回答、决定哪一份发布物值得再采用一次、失败时怎么写日志。NAPI 调用和文件路径在 `AccountSessionAdoption.ets`。
 */

type JsonObject = Record<string, unknown>;

function jsonObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/** 镜像里两份会话：登录会话 `account-session.json` 与本机匿名账号的 `anonymous-session.json`，名字与 `SharedStateMirrorPolicy` 的条目名相同。匿名会话连同匿名账号的身份 `anonymous-account.json` 一起交给 Rust，身份跟着会话走（迁移差异 M40）。 */
export const ADOPTED_SESSIONS: readonly AdoptedSession[] = [
  { item: "account", entry: "account-session", identityEntry: null },
  { item: "anonymous", entry: "anonymous-session", identityEntry: "anonymous-account" },
];

export interface AdoptedSession {
  /** `session_adopt` 的 `item`。 */
  readonly item: string;
  /** 镜像清单里的条目名。 */
  readonly entry: string;
  /** 跟着这份会话一起采用的身份在镜像清单里的条目名，没有时为 null。 */
  readonly identityEntry: string | null;
}

/** 决定要不要再采用一次时用的指纹：会话的指纹，有身份时再接上身份的指纹（清单里没有这一条时按「没有」算），两者任一变了都再采用一次。 */
export function adoptionFingerprint(
  sessionFingerprint: string,
  identityFingerprint: string | null,
): string {
  return identityFingerprint === null
    ? sessionFingerprint
    : `${sessionFingerprint}+${identityFingerprint}`;
}

/**
 * `msime_client_account` 的 `session_adopt` 请求。`publishedPath` 是发布方那份会话文件的绝对路径；发布方的清单说它没有这份会话时为 null，Rust 据此按「发布方没有」处理（登录会话在 `from:"settings"` 时清掉本地的，匿名会话一律不动）。`from` 是发布方：键盘采用设置应用发布的是 `settings`，设置应用采用键盘发布的是 `keyboard`。`identityPath` 是发布方匿名账号身份文件的绝对路径，只随匿名会话给（登录会话为 null）；文件不在就是发布方没有身份，Rust 在采用匿名会话时把身份一起换成发布方的，发布方没有身份而换了账号时删掉本地的，会话和身份始终属于同一个匿名账号。
 */
export function sessionAdoptRequest(
  directory: string,
  publishedPath: string | null,
  item: string,
  from: string,
  identityPath: string | null = null,
): string {
  const action: JsonObject = { operation: "session_adopt", item, from };
  if (publishedPath !== null) action.path = publishedPath;
  if (identityPath !== null) action.identity_path = identityPath;
  return JSON.stringify({ directory, action });
}

/** `session_adopt` 的结果：`adopted`、`unchanged`、`cleared` 之一；失败时 `outcome` 为空串，`error` 是错误码。`localUnreadable` 是 Rust 回答里的 `local_unreadable`：本地会话读不了、发布的这份按规则没有用，这个 `unchanged` 不只取决于发布物。 */
export interface AdoptionResult {
  readonly outcome: string;
  readonly error: string;
  readonly localUnreadable: boolean;
}

const OUTCOMES: readonly string[] = ["adopted", "unchanged", "cleared"];

export function adoptionResult(reply: string): AdoptionResult {
  let envelope: unknown;
  try {
    envelope = JSON.parse(reply);
  } catch {
    return { outcome: "", error: "account_unavailable", localUnreadable: false };
  }
  if (!jsonObject(envelope)) return { outcome: "", error: "account_unavailable", localUnreadable: false };
  if (envelope.ok !== true) {
    const error: unknown = envelope.error;
    return {
      outcome: "",
      error: typeof error === "string" && error.length > 0 ? error : "account_unavailable",
      localUnreadable: false,
    };
  }
  const value: unknown = envelope.value;
  const outcome: unknown = jsonObject(value) ? value.outcome : undefined;
  return typeof outcome === "string" && OUTCOMES.includes(outcome)
    ? { outcome, error: "", localUnreadable: jsonObject(value) && value.local_unreadable === true }
    : { outcome: "", error: "account_unavailable", localUnreadable: false };
}

/**
 * 采用失败时写进日志的那一句，同一份发布物只写一次（键盘每次获得焦点都会再试）。
 *
 * `account_storage` 单独说明：Rust 读发布文件时要求文件属主是本进程的有效 uid、不看所在目录的属主，并拒绝其他用户位（迁移差异 M1）。这条规则假定设置应用和键盘两个进程的 uid 相同，真机上还没有核对过；不同时每次采用都是这个码，键盘永远拿不到设置应用里的登录，所以这句要写清楚是哪一条规则、该去查什么。
 */
export function adoptionFailureMessage(item: string, from: string, error: string): string {
  if (error === "account_storage") {
    return (
      `session adopt from ${from} refused for ${item}: account_storage. ` +
      "The published session file failed Rust's ownership or permission check (owner must be this process's uid, no other-user bits); " +
      "check `ls -ln` of the shared directory and its account-session.json on the device (migration difference M1)"
    );
  }
  return `session adopt from ${from} refused for ${item}: ${error}`;
}

/**
 * 记住每份发布物最后一次成功采用时的指纹，指纹没变就不再调 Rust：同一份发布物再采用一次，结论只能是 `unchanged`，而每次调用都要拿一次 `account-refresh.lock`。失败的不记，下一次照样再试；同一份发布物的失败只报告一次。
 *
 * 本地会话读不了时的 `unchanged`（Rust 带 `local_unreadable`）也不记：那个结论取决于本地此刻读不读得了，不是发布物本身。记下的话本地恢复可读以后，键盘那份刷新过的会话在它再次发布之前一直不被采用，设置应用拿已经被键盘用过的刷新令牌去刷新，超过服务端 30 秒宽限就是重放，两个沙箱一起登出。
 */
export class AdoptionTracker {
  private readonly adopted: Map<string, string> = new Map<string, string>();
  private readonly reported: Map<string, string> = new Map<string, string>();

  /** 这份发布物（发布方、条目、指纹）还要不要采用。 */
  due(from: string, item: string, fingerprint: string): boolean {
    return this.adopted.get(`${from}/${item}`) !== fingerprint;
  }

  succeeded(from: string, item: string, fingerprint: string): void {
    this.adopted.set(`${from}/${item}`, fingerprint);
    this.reported.delete(`${from}/${item}`);
  }

  /** Rust 给出了结论（`result.outcome` 非空）时调：本地读不了而没有采用的不记指纹，下一次照样再调 Rust。 */
  answered(from: string, item: string, fingerprint: string, result: AdoptionResult): void {
    if (result.localUnreadable) {
      this.reported.delete(`${from}/${item}`);
      return;
    }
    this.succeeded(from, item, fingerprint);
  }

  /** 记下一次失败；这份发布物第一次失败时返回要写的日志，之后返回 null。 */
  failed(from: string, item: string, fingerprint: string, error: string): string | null {
    const key: string = `${from}/${item}`;
    const marker: string = `${fingerprint}/${error}`;
    if (this.reported.get(key) === marker) return null;
    this.reported.set(key, marker);
    return adoptionFailureMessage(item, from, error);
  }
}

/**
 * 一条只进不退的异步队列：`run` 把任务排在已有任务之后，任务的失败不传给后面的任务；`settled` 给出「此刻已经排进来的任务都做完」的 promise。
 */
export class SerialGate {
  private tail: Promise<void> = Promise.resolve();

  run(task: () => Promise<void>): Promise<void> {
    const next: Promise<void> = this.tail.then(task).catch((): void => undefined);
    this.tail = next;
    return next;
  }

  settled(): Promise<void> {
    return this.tail;
  }
}

/**
 * 本进程的采用队列。进程启动时 `register` 登记本进程怎么采用另一边的会话并立即排一次；之后每个认证请求的 `beforeRequest` 都再排一次、等它做完，而不是只等此刻已经排进来的。
 *
 * 原来只在启动和回到前台的生命周期事件里排采用，认证请求只等已经排进来的那些。设置页的网页在 `visibilitychange` 时就发认证请求（打字统计补传），它和 ArkTS 窗口的 ACTIVE 事件谁先到没有任何保证；请求先到时它不等前台采用，拿键盘已经轮换掉的刷新令牌去刷新，超过服务端 30 秒宽限就是重放，两个沙箱一起登出。2in1 上设置窗口一直可见、只是换焦点时连生命周期事件都没有。每个请求自己排一次就与事件顺序无关；发布物没变时 `AdoptionTracker` 按指纹跳过，只多读一次清单文件，不调 Rust、不拿锁。
 */
export class AdoptionGate {
  private readonly gate: SerialGate = new SerialGate();
  private adoption: (() => Promise<void>) | null = null;

  /** 登记本进程的采用并立即排一次。 */
  register(adoption: () => Promise<void>): void {
    this.adoption = adoption;
    this.gate.run(adoption);
  }

  /** 生命周期事件里额外排的采用（回到前台、键盘获得焦点），让本地状态尽早跟上；认证请求的顺序不靠它。 */
  run(task: () => Promise<void>): Promise<void> {
    return this.gate.run(task);
  }

  /** 认证请求之前：再排一次登记过的采用，等它和排在它前面的都做完。还没有登记时（两个 ability 在 `onCreate` 里就登记，正常不会出现）只等已经排进来的。 */
  beforeRequest(): Promise<void> {
    const adoption: (() => Promise<void>) | null = this.adoption;
    return adoption === null ? this.gate.settled() : this.gate.run(adoption);
  }
}
