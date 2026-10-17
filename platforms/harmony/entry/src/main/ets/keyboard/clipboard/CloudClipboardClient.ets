import client from 'libmsimeclient.so';
import { AccountSessionAdoption } from '../../account/AccountSessionAdoption';
import { HarmonyAccountIdentity } from '../../account/HarmonyAccountIdentity';
import { RequestPlan, cloudClipboardRequest, statusUserId } from '../../account/HarmonyAccountRequests';
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
 * 键盘扩展眼中的账号云剪贴板（#8201 第 2 步）。
 *
 * 会话、刷新和端到端加密都在 Rust（`msime_client_account_cloud_clipboard`，目录是键盘沙箱自己的 `files/state`，设置应用里的登录经会话镜像采用过来）。每个请求之前先等本进程的会话镜像做完，免得拿已经被设置应用用过的刷新令牌去刷新。键盘只用 `list` 和 `add`；设备的批准、拒绝、移除和重置在设置页里做。
 *
 * 列表绑定的是读到它时登录的账号 id：退出登录或换了账号以后旧列表和待发送项作废，同一个账号的令牌轮换或重新登录不再作废它，与 Rust 只按 user.id 绑定一致（迁移差异 M29）。
 */
export class CloudClipboardClient {
  private readonly stateDirectory: string;

  constructor(stateDirectory: string) {
    this.stateDirectory = stateDirectory;
  }

  /** 没有登录时只读本地会话文件，不联网。 */
  async list(): Promise<CloudClipboardBoundListing | null> {
    if (this.stateDirectory.length === 0) return { state: CloudClipboardState.FAILED, items: [], binding: null };
    await AccountSessionAdoption.beforeRequest();
    const before: string | null = this.userId();
    if (before === null) return { state: CloudClipboardState.SIGNED_OUT, items: [], binding: null };
    const action: ListAction = { operation: 'list', search: '' };
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
    await AccountSessionAdoption.beforeRequest();
    const current: string | null = this.userId();
    if (current === null) return CloudClipboardSendOutcome.SIGNED_OUT;
    if (current !== expectedBinding) return CloudClipboardSendOutcome.FAILED;
    const action: AddAction = { operation: 'add', text: text };
    const outcome: CloudClipboardSendOutcome = CloudClipboardPolicy.parseSend(await this.send(JSON.stringify(action)));
    return this.userId() === null ? CloudClipboardSendOutcome.FAILED : outcome;
  }

  async matches(binding: string): Promise<boolean> {
    if (this.stateDirectory.length === 0 || binding.length === 0) return false;
    return this.userId() === binding;
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
    const plan: RequestPlan = cloudClipboardRequest(this.stateDirectory, HarmonyAccountIdentity.deviceName(), action);
    if (plan.refusal.length > 0) return plan.refusal;
    try {
      return await client.accountCloudClipboard(plan.request);
    } catch (error) {
      KeyboardLog.error(`cloud clipboard request failed: ${describeError(error)}`);
      return '{"ok":false,"error":"account_unavailable"}';
    }
  }
}
