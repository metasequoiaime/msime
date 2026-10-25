import preferences from '@ohos.data.preferences';
import common from '@ohos.app.ability.common';
import userAuth from '@ohos.userIAM.userAuth';
import cryptoFramework from '@ohos.security.cryptoFramework';
import { BusinessError } from '@kit.BasicServicesKit';

/** 只接受系统验证成功或明确未设置锁屏密码，不记录认证令牌。 */
export class StatisticsAuthentication {
  private timer: number = -1;
  private instance: userAuth.UserAuthInstance | undefined = undefined;
  private finish: ((success: boolean) => void) | undefined = undefined;

  enabled(context: common.UIAbilityContext): boolean {
    return preferences.getPreferencesSync(context, { name: 'statistics-authentication' })
      .getSync('enabled', true) as boolean;
  }

  async setEnabled(context: common.UIAbilityContext, enabled: boolean): Promise<void> {
    const store = preferences.getPreferencesSync(context, { name: 'statistics-authentication' });
    const previous = store.getSync('enabled', true);
    store.putSync('enabled', enabled);
    try {
      await store.flush();
    } catch (error) {
      store.putSync('enabled', previous);
      throw error as Error;
    }
  }

  authenticate(context: common.UIAbilityContext): Promise<boolean> {
    this.cancel();
    if (!this.enabled(context)) return Promise.resolve(true);
    try {
      userAuth.getAvailableStatus(userAuth.UserAuthType.PIN, userAuth.AuthTrustLevel.ATL1);
    } catch (error) {
      return Promise.resolve((error as BusinessError).code === userAuth.UserAuthResultCode.NOT_ENROLLED);
    }
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
            // 生物认证不可用时仍保留系统密码，不把它当成免认证。
          }
        }
        const parameters: userAuth.AuthParam = {
          // 真机的认证窗口拒绝空挑战值，使用系统安全随机源生成本次请求的挑战。
          challenge: cryptoFramework.createRandom().generateRandomSync(16).data,
          authType: types, authTrustLevel: userAuth.AuthTrustLevel.ATL1
        };
        const widget: userAuth.WidgetParam = { title: '查看打字统计' };
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
      } catch {
        this.cancel();
        resolve(false);
      }
    });
  }

  cancel(): void {
    if (this.timer !== -1) clearTimeout(this.timer);
    this.timer = -1;
    const instance = this.instance;
    const finish = this.finish;
    this.instance = undefined;
    this.finish = undefined;
    if (instance !== undefined) {
      try { instance.cancel(); } catch { /* 系统已结束的请求同样失效。 */ }
      try { instance.off('result'); } catch { /* 解绑失败不能恢复授权。 */ }
    }
    finish?.(false);
  }
}
