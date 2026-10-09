import { socket } from '@kit.NetworkKit';
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';
import common from '@ohos.app.ability.common';
import client from 'libmsimeclient.so';
import { GoogleLoopbackRequestHead } from './GoogleLoopbackRequestHead';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';

/** 监听端口从动态端口段里随机挑，被占用就换一个，最多试这么多次。 */
const PORT_ATTEMPTS: number = 8;
const PORT_FIRST: number = 49152;
const PORT_SPAN: number = 65536 - PORT_FIRST;

interface NativeReply<T> {
  ok: boolean;
  value?: T;
  error?: string;
}

interface TargetRequest {
  operation: string;
  port: number;
}

interface PlanRequest {
  operation: string;
  authorization_url: string;
  target: string;
  expires_in: number;
}

interface ReplyRequest {
  operation: string;
  head: string | null;
  state: string;
}

interface TargetValue {
  target: string;
}

interface PlanValue {
  state: string;
  wait_ms: number;
  max_request_bytes: number;
  io_timeout_ms: number;
}

interface CallbackValue {
  outcome: string;
  code?: string;
  error?: string;
  response: string;
}

interface ChallengeValue {
  challenge_id?: string;
  expires_in?: number;
  authorization_url?: string;
}

interface AccountReply {
  ok?: boolean;
  value?: ChallengeValue;
  error?: string;
}

/** 回环等待的结果：拿到授权码，或以某个 `account_*` 错误结束。 */
class LoopbackOutcome {
  code: string = '';
  error: string = '';
}

/**
 * 鸿蒙设置应用里的 Google 登录：系统浏览器加本机回环回跳（RFC 8252），与桌面端 `AccountSession::sign_in_google_with_browser` 是同一条流程，后端也是同一套接口。
 *
 * 鸿蒙没有 Google 服务，GoogleSignIn SDK 用不了；Google 也拒绝在应用内的 WebView 里登录。所以这里在 127.0.0.1 上监听，用 `openLink` 把后端生成的授权链接交给系统浏览器，等 Google 跳回本机端口。哪些判定不在这里写：回跳地址的形状、授权链接能不能打开、等多久、一次请求是不是回跳、写回浏览器的页面，都由 `msime_client_google_loopback` 给出，与桌面端共用一份；这里只做 socket、浏览器和后台保活。
 *
 * 用户在浏览器里登录时设置应用在后台，系统可能冻结它，回跳就没人接。所以等待期间申请延迟挂起（`requestSuspendDelay`）；申请不到时照样等，在模拟器上不申请也能收到回跳，真机上不一定。
 */
export class HarmonyGoogleSignIn {
  private readonly context: common.UIAbilityContext;
  private readonly account: (action: string) => Promise<string>;
  /** 每次登录或取消都加一，过期流程的结果被丢掉。 */
  private generation: number = 0;
  /** 正在等回跳的那次登录的结束函数；取消时用它立刻结束等待。 */
  private finish: ((outcome: LoopbackOutcome) => void) | null = null;

  constructor(context: common.UIAbilityContext, account: (action: string) => Promise<string>) {
    this.context = context;
    this.account = account;
  }

  /** 完成一次 Google 登录，返回与账号桥 `login` 同形状的回复文档。新的一次会结束还在进行的上一次。 */
  async signIn(): Promise<string> {
    this.cancel();
    const generation: number = ++this.generation;
    KeyboardLog.info(`google sign-in: started (${generation})`);
    const server: socket.TCPSocketServer = socket.constructTCPSocketServerInstance();
    let suspendId: number = -1;
    try {
      const port: number = await this.listen(server);
      if (port < 0) {
        KeyboardLog.warn('google sign-in: no loopback port could be bound');
        return this.failure('account_unavailable');
      }
      const targetRequest: TargetRequest = { operation: 'target', port: port };
      const target: TargetValue | null = this.native<TargetValue>(targetRequest);
      if (target === null) {
        return this.failure('account_unavailable');
      }
      const challengeDocument: string = await this.account(JSON.stringify({
        operation: 'request_code', provider: 'google', target: target.target
      }));
      const challenge: AccountReply = JSON.parse(challengeDocument) as AccountReply;
      if (challenge.ok !== true || challenge.value === undefined) {
        KeyboardLog.warn(`google sign-in: challenge refused ${challenge.error ?? '-'}`);
        return challengeDocument;
      }
      if (generation !== this.generation) {
        return this.failure('account_cancelled');
      }
      const challengeId: string = challenge.value.challenge_id ?? '';
      const authorizationUrl: string = challenge.value.authorization_url ?? '';
      const planRequest: PlanRequest = {
        operation: 'plan',
        authorization_url: authorizationUrl,
        target: target.target,
        expires_in: challenge.value.expires_in ?? 0
      };
      const plan: PlanValue | null = this.native<PlanValue>(planRequest);
      if (plan === null || challengeId.length === 0) {
        KeyboardLog.warn('google sign-in: the authorization URL was not opened');
        return this.failure('account_unavailable');
      }
      suspendId = this.keepAlive();
      // 先挂好回跳的处理再打开浏览器，回跳来得再快也不会错过。
      const waiting: Promise<LoopbackOutcome> = this.waitForCallback(server, plan);
      try {
        await this.context.openLink(authorizationUrl);
      } catch (error) {
        KeyboardLog.error(`google sign-in could not open the browser: ${describeError(error)}`);
        this.settle(this.outcome('', 'account_unavailable'));
      }
      const outcome: LoopbackOutcome = await waiting;
      if (outcome.error.length > 0) {
        // 等待以取消结束包括超时：challenge 寿命内浏览器没有跳回来。
        KeyboardLog.warn(`google sign-in: no authorization code (${outcome.error})`);
        return this.failure(outcome.error);
      }
      if (generation !== this.generation) {
        return this.failure('account_cancelled');
      }
      const login: string = await this.account(JSON.stringify({
        operation: 'google_login', challenge_id: challengeId, code: outcome.code
      }));
      const reply: AccountReply = JSON.parse(login) as AccountReply;
      if (reply.ok === true) {
        KeyboardLog.info('google sign-in: signed in');
      } else {
        KeyboardLog.warn(`google sign-in: the code reached the backend and was refused (${reply.error ?? '-'})`);
      }
      return login;
    } finally {
      if (suspendId >= 0) {
        try {
          backgroundTaskManager.cancelSuspendDelay(suspendId);
        } catch (error) {
          KeyboardLog.warn(`google sign-in suspend delay not cancelled: ${describeError(error)}`);
        }
      }
      server.off('connect');
      server.close().catch((error: Object): void => {
        KeyboardLog.warn(`google sign-in listener not closed: ${describeError(error)}`);
      });
      if (generation === this.generation) {
        this.finish = null;
      }
    }
  }

  /** 结束正在进行的登录，它随即以 `account_cancelled` 结束。浏览器标签页关掉时应用无从得知，所以页面提供取消，而不是让用户等到超时。没有登录在进行时什么也不做。 */
  cancel(): void {
    this.generation++;
    this.settle(this.outcome('', 'account_cancelled'));
  }

  private settle(outcome: LoopbackOutcome): void {
    const finish: ((outcome: LoopbackOutcome) => void) | null = this.finish;
    this.finish = null;
    if (finish !== null) {
      finish(outcome);
    }
  }

  /** 在 127.0.0.1 的一个随机动态端口上监听，返回端口；都失败时返回 -1。 */
  private async listen(server: socket.TCPSocketServer): Promise<number> {
    for (let attempt = 0; attempt < PORT_ATTEMPTS; attempt++) {
      const port: number = PORT_FIRST + Math.floor(Math.random() * PORT_SPAN);
      try {
        await server.listen({ address: '127.0.0.1', port: port, family: 1 });
        return port;
      } catch (error) {
        KeyboardLog.warn(`google sign-in port ${port} unavailable: ${describeError(error)}`);
      }
    }
    return -1;
  }

  /** 等回跳：每个连接读到请求头就交给 reply 判定并把页面写回去；拿到授权码、用户在 Google 页面拒绝、超时或取消时结束。超时与桌面端一样算作取消。 */
  private waitForCallback(server: socket.TCPSocketServer, plan: PlanValue): Promise<LoopbackOutcome> {
    return new Promise<LoopbackOutcome>((resolve: (outcome: LoopbackOutcome) => void): void => {
      const deadline: number = setTimeout((): void => {
        this.settle(this.outcome('', 'account_cancelled'));
      }, plan.wait_ms);
      this.finish = (outcome: LoopbackOutcome): void => {
        clearTimeout(deadline);
        resolve(outcome);
      };
      server.on('connect', (connection: socket.TCPSocketConnection): void => {
        this.answer(connection, plan);
      });
    });
  }

  private answer(connection: socket.TCPSocketConnection, plan: PlanValue): void {
    const head: GoogleLoopbackRequestHead = new GoogleLoopbackRequestHead(plan.max_request_bytes);
    let answered: boolean = false;
    let slow: number = -1;
    const reply = (): void => {
      if (answered) {
        return;
      }
      answered = true;
      clearTimeout(slow);
      const replyRequest: ReplyRequest = { operation: 'reply', head: head.head(), state: plan.state };
      const decision: CallbackValue | null = this.native<CallbackValue>(replyRequest);
      if (decision === null) {
        connection.close().catch((): void => {});
        return;
      }
      // 浏览器上的页面只是告知；结果不取决于它有没有送达。
      connection.send({ data: decision.response })
        .catch((): void => {})
        .finally((): void => {
          connection.close().catch((): void => {});
        });
      if (decision.outcome === 'code' && decision.code !== undefined) {
        this.settle(this.outcome(decision.code, ''));
      } else if (decision.outcome === 'failed') {
        this.settle(this.outcome('', decision.error ?? 'account_unavailable'));
      }
    };
    // 一个连接迟迟不发完请求头（空闲的预连接、慢吞吞的本机客户端）时按没读到回复，不让它占着。
    slow = setTimeout(reply, plan.io_timeout_ms);
    connection.on('message', (message: socket.SocketMessageInfo): void => {
      if (head.push(new Uint8Array(message.message))) {
        reply();
      }
    });
  }

  /** 申请延迟挂起，返回 requestId；申请不到时返回 -1，照样继续。 */
  private keepAlive(): number {
    try {
      return backgroundTaskManager.requestSuspendDelay('Google 登录：等待浏览器跳回', (): void => {
        KeyboardLog.warn('google sign-in suspend delay is about to expire');
      }).requestId;
    } catch (error) {
      KeyboardLog.warn(`google sign-in suspend delay unavailable: ${describeError(error)}`);
      return -1;
    }
  }

  private native<T>(request: Object): T | null {
    try {
      const reply: NativeReply<T> = JSON.parse(client.googleLoopback(JSON.stringify(request))) as NativeReply<T>;
      if (reply.ok !== true || reply.value === undefined) {
        KeyboardLog.warn(`google sign-in refused: ${reply.error ?? 'no value'}`);
        return null;
      }
      return reply.value;
    } catch (error) {
      KeyboardLog.error(`google sign-in native call failed: ${describeError(error)}`);
      return null;
    }
  }

  private outcome(code: string, error: string): LoopbackOutcome {
    const value: LoopbackOutcome = new LoopbackOutcome();
    value.code = code;
    value.error = error;
    return value;
  }

  private failure(code: string): string {
    return JSON.stringify({ ok: false, error: code });
  }
}
