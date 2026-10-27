import client from 'libmsimeclient.so';
import { AccountSessionAdoption } from '../../account/AccountSessionAdoption';
import { HarmonyPrivacyConsent } from '../../privacy/HarmonyPrivacyConsent';
import { PrivacyConsentPolicy } from '../../privacy/PrivacyConsentPolicy';
import { HarmonyAccountIdentity } from '../../account/HarmonyAccountIdentity';
import {
  CloudClipboardRoute, RequestPlan, cloudClipboardRequest, cloudClipboardRoute, relayBinding, relayBindingRevision,
  relayCloudClipboardRequest, statusUserId
} from '../../account/HarmonyAccountRequests';
import { KeyboardLog, describeError } from '../KeyboardLog';
import {
  CloudClipboardListing, CloudClipboardPolicy, CloudClipboardSendOutcome, CloudClipboardState
} from './CloudClipboardPolicy';

export interface CloudClipboardBoundListing extends CloudClipboardListing {
  readonly binding: string | null;
}

interface ListAction {
  operation: string;
  search: string;
}

interface AddAction {
  operation: string;
  text: string;
}

/**
 * 键盘扩展眼中的云剪贴板（#8201 第 2 步）。
 *
 * 每个请求先问共享层该走哪条路（`msime_client_cloud_clipboard_source`，读的是设置应用经共享沙箱镜像过来的偏好）：配置了自建中继或 WebDAV 网盘就只走它（`msime_client_relay_cloud_clipboard`），失败、来源读不出都按失败处理，从不退回水杉账号；没有配置时走账号（`msime_client_account_cloud_clipboard`），请求带上偏好目录，Rust 发出前发现已经改成中继也会拒绝。会话、刷新、端到端加密和网盘日志都在 Rust 里，目录是键盘沙箱自己的 `files/state`，设置应用里的登录经会话镜像采用过来。账号请求之前先等本进程的会话镜像做完，免得拿已经被设置应用用过的刷新令牌去刷新。键盘只用 `list` 和 `add`；设备的批准、拒绝、移除和重置在设置页里做。
 *
 * 两条路都只用设置应用生成的设备身份（请求带 `create_identity: false`）：密钥串由共享沙箱从设置应用镜像过来（`SharedStateMirrorPolicy.CLOUD_CLIPBOARD_IDENTITY_ENTRIES`），本机还没有时 Rust 答 `clipboard_identity_missing`、不登记，同一台手机在空间里只是一台设备，只需批准一次。
 *
 * 账号路径的列表绑定读到它时登录的账号 id：退出登录或换了账号以后旧列表和待发送项作废，同一个账号的令牌轮换或重新登录不再作废它，与 Rust 只按 user.id 绑定一致（迁移差异 M29）。中继路径绑定读到它时的偏好修订号（`relayBinding`），换了中继、网盘或改回账号以后旧列表作废。
 */
export class CloudClipboardClient {
  private readonly stateDirectory: string;

  constructor(stateDirectory: string) {
    this.stateDirectory = stateDirectory;
  }

  /** 走账号且没有登录时只读本地会话文件，不联网。 */
  async list(): Promise<CloudClipboardBoundListing | null> {
    if (this.stateDirectory.length === 0) return { state: CloudClipboardState.FAILED, items: [], binding: null };
    const action: ListAction = { operation: 'list', search: '' };
    const route: CloudClipboardRoute = this.route();
    if (route.source === 'failed') return { state: CloudClipboardState.FAILED, items: [], binding: null };
    if (route.source === 'relay') {
      const binding: string = relayBinding(route);
      const relayed: CloudClipboardListing =
        CloudClipboardPolicy.parseList(await this.sendRelay(JSON.stringify(action), route.revision));
      // 读的时候偏好换过了：这一页不属于现在那条路。
      if (this.currentBinding() !== binding) return null;
      return { state: relayed.state, items: relayed.items, binding: binding };
    }
    await AccountSessionAdoption.beforeRequest();
    const before: string | null = this.userId();
    if (before === null) return { state: CloudClipboardState.SIGNED_OUT, items: [], binding: null };
    const listing: CloudClipboardListing = CloudClipboardPolicy.parseList(await this.send(JSON.stringify(action)));
    const after: string | null = this.userId();
    if (after !== before) {
      if (after !== null) return null;
      const signedOut: CloudClipboardBoundListing = { state: CloudClipboardState.SIGNED_OUT, items: [], binding: null };
      return signedOut;
    }
    return { state: listing.state, items: listing.items, binding: before };
  }

  async add(text: string, expectedBinding: string): Promise<CloudClipboardSendOutcome> {
    if (!CloudClipboardPolicy.validText(text)) return CloudClipboardSendOutcome.INVALID;
    if (this.stateDirectory.length === 0 || expectedBinding.length === 0) return CloudClipboardSendOutcome.FAILED;
    const action: AddAction = { operation: 'add', text: text };
    const revision: number = relayBindingRevision(expectedBinding);
    if (revision >= 0) {
      // 列表读自中继或网盘：只发给同一份配置，Rust 读配置时再按修订号比较一次。
      if (relayBinding(this.route()) !== expectedBinding) return CloudClipboardSendOutcome.FAILED;
      return CloudClipboardPolicy.parseSend(await this.sendRelay(JSON.stringify(action), revision));
    }
    if (this.route().source !== 'account') return CloudClipboardSendOutcome.FAILED;
    await AccountSessionAdoption.beforeRequest();
    const current: string | null = this.userId();
    if (current === null) return CloudClipboardSendOutcome.SIGNED_OUT;
    if (current !== expectedBinding) return CloudClipboardSendOutcome.FAILED;
    const outcome: CloudClipboardSendOutcome = CloudClipboardPolicy.parseSend(await this.send(JSON.stringify(action)));
    return this.userId() === null ? CloudClipboardSendOutcome.FAILED : outcome;
  }

  async matches(binding: string): Promise<boolean> {
    if (this.stateDirectory.length === 0 || binding.length === 0) return false;
    return this.currentBinding() === binding;
  }

  /** 现在这条路的绑定：中继或网盘是 `relayBinding`，账号是登录的账号 id；没有登录或来源读不出时为 null。 */
  private currentBinding(): string | null {
    const route: CloudClipboardRoute = this.route();
    if (route.source === 'relay') return relayBinding(route);
    return route.source === 'account' ? this.userId() : null;
  }

  /** 只读偏好、不联网，可以在主线程上调用。 */
  private route(): CloudClipboardRoute {
    try {
      return cloudClipboardRoute(client.cloudClipboardSource(this.stateDirectory));
    } catch (error) {
      KeyboardLog.error(`cloud clipboard: source unreadable: ${describeError(error)}`);
      return cloudClipboardRoute('');
    }
  }

  private userId(): string | null {
    try {
      return statusUserId(client.accountStatus(this.stateDirectory));
    } catch (error) {
      KeyboardLog.error(`cloud clipboard: account status unreadable: ${describeError(error)}`);
      return null;
    }
  }

  private async send(action: string): Promise<string> {
    // 云剪贴板要联网，没有隐私政策的授权就按请求失败处理，面板照常显示本机剪贴板历史。
    if (!HarmonyPrivacyConsent.networkAllowed()) return PrivacyConsentPolicy.refusal();
    const plan: RequestPlan = cloudClipboardRequest(this.stateDirectory, HarmonyAccountIdentity.deviceName(), action,
      this.stateDirectory, false);
    if (plan.refusal.length > 0) return plan.refusal;
    try {
      return await client.accountCloudClipboard(plan.request);
    } catch (error) {
      KeyboardLog.error(`cloud clipboard request failed: ${describeError(error)}`);
      return '{"ok":false,"error":"account_unavailable"}';
    }
  }

  /** 经自建中继或 WebDAV 网盘发一个动作。`revision` 是读来源时的偏好修订号。 */
  private async sendRelay(action: string, revision: number): Promise<string> {
    // 与账号路径同一道门：没有隐私政策的授权，中继和网盘也一个请求都不发。
    if (!HarmonyPrivacyConsent.networkAllowed()) return PrivacyConsentPolicy.refusal();
    const plan: RequestPlan = relayCloudClipboardRequest(this.stateDirectory, this.stateDirectory,
      HarmonyAccountIdentity.deviceName(), action, revision, false);
    if (plan.refusal.length > 0) return plan.refusal;
    try {
      return await client.relayCloudClipboard(plan.request);
    } catch (error) {
      KeyboardLog.error(`cloud clipboard relay request failed: ${describeError(error)}`);
      return '{"ok":false,"error":"relay_unavailable"}';
    }
  }
}
