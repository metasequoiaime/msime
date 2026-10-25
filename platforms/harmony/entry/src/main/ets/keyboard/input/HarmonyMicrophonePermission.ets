import abilityAccessCtrl from '@ohos.abilityAccessCtrl';
import bundleManager from '@ohos.bundle.bundleManager';
import common from '@ohos.app.ability.common';
import { KeyboardLog, describeError } from '../KeyboardLog';

/** 输入法扩展只能检查授权；系统授权窗口必须由本应用的 UIAbility 发起。 */
export class HarmonyMicrophonePermission {
  static granted(): boolean {
    try {
      const info = bundleManager.getBundleInfoForSelfSync(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_WITH_APPLICATION);
      return abilityAccessCtrl.createAtManager().checkAccessTokenSync(
        info.appInfo.accessTokenId, 'ohos.permission.MICROPHONE') ===
        abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED;
    } catch (error) {
      KeyboardLog.error(`麦克风权限检查失败：${describeError(error)}`);
      return false;
    }
  }

  static async request(context: common.UIAbilityContext): Promise<void> {
    if (HarmonyMicrophonePermission.granted()) return;
    try {
      await abilityAccessCtrl.createAtManager().requestPermissionsFromUser(
        context, ['ohos.permission.MICROPHONE']);
    } catch (error) {
      KeyboardLog.error(`麦克风授权窗口无法打开：${describeError(error)}`);
    }
  }
}
