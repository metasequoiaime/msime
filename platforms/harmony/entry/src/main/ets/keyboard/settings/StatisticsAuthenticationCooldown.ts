/** 认证成功、或在已认证状态下离开统计页（含退到后台）之后，这么久之内再进入不用重新认证。 */
export const STATISTICS_AUTHENTICATION_COOLDOWN_MS: number = 5 * 60 * 1000;

/** 只在内存里记时刻。`now` 返回含深度睡眠的开机毫秒数（`systemDateTime.getUptime(STARTUP)`），应用被结束或设备重启后重新认证，改系统时间也绕不过去。 */
export class StatisticsAuthenticationCooldown {
  private readonly now: () => number;
  private lastAuthorized: number = -1;
  private authorized: boolean = false;

  constructor(now: () => number) {
    this.now = now;
  }

  /** 系统认证成功，或冷却期内放行：本次页面会话已授权。关闭开关和未设锁屏密码得到的放行不调它，不开启冷却。 */
  granted(): void {
    this.authorized = true;
    this.lastAuthorized = this.now();
  }

  /** 离开页面、退到后台或开始新的认证：已授权时把冷却的起点挪到现在。 */
  ended(): void {
    if (this.authorized) this.lastAuthorized = this.now();
    this.authorized = false;
  }

  active(): boolean {
    if (this.lastAuthorized < 0) return false;
    const elapsed: number = this.now() - this.lastAuthorized;
    return elapsed >= 0 && elapsed < STATISTICS_AUTHENTICATION_COOLDOWN_MS;
  }
}
