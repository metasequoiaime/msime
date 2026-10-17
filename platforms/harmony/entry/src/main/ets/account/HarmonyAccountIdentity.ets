import bundleManager from '@ohos.bundle.bundleManager';
import deviceInfo from '@ohos.deviceInfo';
import client from 'libmsimeclient.so';
import { AppEdition } from '../keyboard/AppEdition';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import { OsVersionPolicy } from '../keyboard/input/OsVersionPolicy';
import { AccountSessionAdoption } from './AccountSessionAdoption';
import {
  anonymousAccountRequest, clientAgentRequest, cloudClipboardDeviceName, replyError, replyValueText
} from './HarmonyAccountRequests';

/**
 * 本宿主在账号服务面前是谁：自报的 User-Agent（服务端「我的设备」据此显示鸿蒙和机型）与端到端加密云剪贴板登记本机时的设备名。设置应用和键盘两个进程各算一次，结果相同。
 */
export class HarmonyAccountIdentity {
  private static agent: string | null = null;

  /** `msime-harmony/<版本> (<机型>; HarmonyOS <版本>; edition=<id>)`，由 Rust 拼装和清洗；拼不出时是空串，请求以默认 User-Agent 发出。 */
  static clientAgent(): string {
    const cached: string | null = HarmonyAccountIdentity.agent;
    if (cached !== null) return cached;
    let version: string = '';
    try {
      version = bundleManager.getBundleInfoForSelfSync(
        bundleManager.BundleFlag.GET_BUNDLE_INFO_DEFAULT).versionName;
    } catch (error) {
      KeyboardLog.error(`client agent: version unreadable: ${describeError(error)}`);
    }
    const reply: string = client.clientAgent(clientAgentRequest(version, deviceInfo.marketName,
      deviceInfo.productModel, OsVersionPolicy.release(deviceInfo.osFullName), AppEdition.current().id));
    const value: string | null = replyValueText(reply);
    const agent: string = value === null ? '' : JSON.parse(value) as string;
    if (value === null) {
      KeyboardLog.error(`client agent refused: ${replyError(reply, 'account_invalid')}`);
    }
    HarmonyAccountIdentity.agent = agent;
    return agent;
  }

  /** 云剪贴板端到端加密登记本机时给其他设备看的名称。 */
  static deviceName(): string {
    return cloudClipboardDeviceName(deviceInfo.marketName, deviceInfo.productModel);
  }

  /**
   * 本机匿名账号还没有可用的会话时注册它；已有会话时只读一次文件、不联网。先等本进程的会话镜像做完：另一边已经注册过的匿名账号要先采用过来，不然这里会再注册一个，用掉每个出口地址每天 5 次的额度。失败只记日志，下一次启动再试。
   */
  static async ensureAnonymous(directory: string): Promise<void> {
    await AccountSessionAdoption.beforeRequest();
    try {
      const reply: string = await client.ensureAnonymousAccountWithAgent(
        anonymousAccountRequest(directory, HarmonyAccountIdentity.clientAgent()));
      const error: string = replyError(reply, 'account_unavailable');
      if (error.length > 0) KeyboardLog.error(`anonymous account: ${error}`);
    } catch (error) {
      KeyboardLog.error(`anonymous account: ${describeError(error)}`);
    }
  }
}
