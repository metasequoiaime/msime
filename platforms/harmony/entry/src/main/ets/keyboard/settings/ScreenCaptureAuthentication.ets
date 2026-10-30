import userAuth from '@ohos.userIAM.userAuth';
import cryptoFramework from '@ohos.security.cryptoFramework';
import { KeyboardLog, describeError } from '../KeyboardLog';

/**
 * 切换「防止截屏」前的系统认证：锁屏密码，加上可用的指纹、人脸，ATL1，与打字统计的认证相同；不带冷却，每次切换都要验证。不记录认证令牌。
 *
 * 与 `StatisticsAuthentication` 不同，没有锁屏密码时不放行：调用方先问 `available()`，不可用就拒绝切换（见 `ScreenCaptureProtectionPolicy.decide`）。
 */
export class ScreenCaptureAuthentication {
  private timer: number = -1;
  private instance: userAuth.UserAuthInstance | undefined = undefined;
  private finish: ((success: boolean) => void) | undefined = undefined;

  /** 设备此刻有没有可用的系统认证：锁屏密码没设置或被锁定时为假。 */
  static available(): boolean {
    try {
      userAuth.getAvailableStatus(userAuth.UserAuthType.PIN, userAuth.AuthTrustLevel.ATL1);
      return true;
    } catch {
      return false;
    }
  }

  /** 弹出系统认证，`enabling` 决定标题；成功时回答真，取消、失败、超时都回答假。 */
  authenticate(enabling: boolean): Promise<boolean> {
    this.cancel();
    return new Promise<boolean>((resolve: (success: boolean) => void): void => {
      let instance: userAuth.UserAuthInstance;
      try {
        const types: userAuth.UserAuthType[] = [userAuth.UserAuthType.PIN];
        const biometrics: userAuth.UserAuthType[] = [userAuth.UserAuthType.FINGERPRINT, userAuth.UserAuthType.FACE];
        for (const type of biometrics) {
          try {
            userAuth.getAvailableStatus(type, userAuth.AuthTrustLevel.ATL1);
            types.push(type);
          } catch {
            // 生物认证不可用时仍保留锁屏密码。
          }
        }
        const parameters: userAuth.AuthParam = {
          // 真机的认证窗口拒绝空挑战值，使用系统安全随机源生成本次请求的挑战。
          challenge: cryptoFramework.createRandom().generateRandomSync(16).data,
          authType: types, authTrustLevel: userAuth.AuthTrustLevel.ATL1
        };
        const widget: userAuth.WidgetParam = { title: enabling ? '打开防止截屏' : '关闭防止截屏' };
        instance = userAuth.getUserAuthInstance(parameters, widget);
        const callback: userAuth.IAuthCallback = {
          onResult: (result: userAuth.UserAuthResult): void => {
            if (this.instance !== instance) return;
            if (this.timer !== -1) clearTimeout(this.timer);
            this.timer = -1;
            this.instance = undefined;
            this.finish = undefined;
            instance.off('result', callback);
            resolve(result.result === userAuth.UserAuthResultCode.SUCCESS);
          }
        };
        this.instance = instance;
        this.finish = resolve;
        instance.on('result', callback);
        this.timer = setTimeout((): void => this.cancel(), 115000);
        instance.start();
      } catch (error) {
        KeyboardLog.error(`screen capture authentication unavailable: ${describeError(error)}`);
        this.cancel();
        resolve(false);
      }
    });
  }

  /** 超时或开始新的认证时结束旧的一次，旧的那次回答假。 */
  cancel(): void {
    if (this.timer !== -1) clearTimeout(this.timer);
    this.timer = -1;
    const instance = this.instance;
    const finish = this.finish;
    this.instance = undefined;
    this.finish = undefined;
    if (instance !== undefined) {
      try { instance.cancel(); } catch { /* 系统已结束的请求同样失效。 */ }
      try { instance.off('result'); } catch { /* 解绑失败不能变成放行。 */ }
    }
    finish?.(false);
  }
}
