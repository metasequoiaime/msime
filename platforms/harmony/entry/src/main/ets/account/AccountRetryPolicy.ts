/**
 * 账号请求在传输层失败后，什么情况下换一条新连接再发一次。
 *
 * 鸿蒙的 HTTP 栈会复用空闲的连接。App 在后台放一会儿，对端（Cloudflare）就把这条连接关了，下一个请求落在它上面，刚发出去就收到断开：curl 报 56（接收失败），也可能是 52（空回复）或 55（发送失败）。模拟器上实测，这样失败的请求 `connect` 和 `tls` 都是 0，36 毫秒就结束，紧接着的请求换了新连接全部成功。「我的」页因此显示「账号服务暂不可用」，其实只要再发一次。
 *
 * 只重试一次，而且只重试幂等的方法（GET、PUT、DELETE）：连接断在发送之后时，服务端可能已经处理了这个请求，POST 和 PATCH 再发一次可能重复提交。连不上、超时这些不是连接复用的问题，重试也只是让失败来得更晚，所以不在其中。
 */

/** 鸿蒙把 curl 的错误码加上 2300000 作为 BusinessError 的 code。 */
const STALE_CONNECTION_CODES: readonly number[] = [
  2300052, // CURLE_GOT_NOTHING
  2300055, // CURLE_SEND_ERROR
  2300056, // CURLE_RECV_ERROR
];

const IDEMPOTENT_METHODS: readonly string[] = ["GET", "PUT", "DELETE"];

/** 传输层抛出的错误是不是落在了一条已经失效的复用连接上。鸿蒙抛的是带数字 `code` 的 BusinessError，不是 Error。 */
export function staleConnectionFailure(cause: unknown): boolean {
  if (cause === null || typeof cause !== "object") return false;
  const code: unknown = (cause as { code?: unknown }).code;
  return typeof code === "number" && STALE_CONNECTION_CODES.includes(code);
}

/** 第 `attempt` 次（从 0 起）以 `method` 发出的请求这样失败后，要不要再发一次。 */
export function retryAccountRequest(method: string, attempt: number, cause: unknown): boolean {
  return attempt === 0 && IDEMPOTENT_METHODS.includes(method) && staleConnectionFailure(cause);
}
