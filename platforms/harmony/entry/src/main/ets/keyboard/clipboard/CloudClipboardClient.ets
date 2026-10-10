import { AccountCloudBridge } from '../../account/AccountCloudBridge';
import { FileSessionStore, HarmonyAccountTransport } from '../../account/HarmonyAccountTransport';
import { sha256Hex } from '../CredentialCrypto';
import {
  CloudClipboardListing, CloudClipboardPolicy, CloudClipboardSendOutcome, CloudClipboardState
} from './CloudClipboardPolicy';

export interface CloudClipboardBoundListing extends CloudClipboardListing {
  readonly binding: string | null;
}

interface CloudClipboardListAction {
  operation: string;
  clipboard_operation: string;
  search: string;
}

/**
 * The keyboard extension's view of the account's cloud clipboard.
 *
 * It holds no credentials. Every operation opens the session file the settings application signed in with, in the `files/state` directory both processes share, and goes through the same `AccountCloudBridge` that page uses, so expiry, single-use refresh tokens and sign-out are handled in one place. Reading the file each time rather than keeping a bridge alive is what makes a sign-out or a different sign-in in the settings page take effect here without any notification between the processes.
 */
export class CloudClipboardClient {
  private readonly stateDirectory: string;
  private readonly transport: HarmonyAccountTransport = new HarmonyAccountTransport();

  constructor(stateDirectory: string) {
    this.stateDirectory = stateDirectory;
  }

  /** A signed-out keyboard answers from the session file alone and never touches the network for this. */
  async list(): Promise<CloudClipboardBoundListing | null> {
    if (this.stateDirectory.length === 0) return { state: CloudClipboardState.FAILED, items: [], binding: null };
    const bridge: AccountCloudBridge = this.bridge();
    if (bridge.currentUserId() === null) return { state: CloudClipboardState.SIGNED_OUT, items: [], binding: null };
    const action: CloudClipboardListAction = { operation: 'clipboard', clipboard_operation: 'list', search: '' };
    const listing: CloudClipboardListing = CloudClipboardPolicy.parseList(await bridge.handle(JSON.stringify(action)));
    const binding: string | null = await this.bindingFor(bridge);
    if (binding === null) {
      if (this.bridge().currentUserId() !== null) return null;
      const signedOut: CloudClipboardBoundListing = { state: CloudClipboardState.SIGNED_OUT,
        items: [], binding: null };
      return signedOut;
    }
    return { state: listing.state, items: listing.items, binding: binding };
  }

  async add(text: string, expectedBinding: string): Promise<CloudClipboardSendOutcome> {
    if (!CloudClipboardPolicy.validText(text)) return CloudClipboardSendOutcome.INVALID;
    if (this.stateDirectory.length === 0 || expectedBinding.length === 0) return CloudClipboardSendOutcome.FAILED;
    const bridge: AccountCloudBridge = this.bridge();
    if (bridge.currentUserId() === null) return CloudClipboardSendOutcome.SIGNED_OUT;
    if (await this.bindingFor(bridge) !== expectedBinding) return CloudClipboardSendOutcome.FAILED;
    const outcome: CloudClipboardSendOutcome = CloudClipboardPolicy.parseSend(
      await bridge.addClipboardForCurrentSession(text));
    return await this.bindingFor(bridge) === null ? CloudClipboardSendOutcome.FAILED : outcome;
  }

  async matches(binding: string): Promise<boolean> {
    if (this.stateDirectory.length === 0 || binding.length === 0) return false;
    return await this.bindingFor(this.bridge()) === binding;
  }

  /** Hash the validated stored document. Only the digest leaves this method; a rotation or new login invalidates old rows. */
  private async bindingFor(bridge: AccountCloudBridge): Promise<string | null> {
    if (!bridge.matchesStoredSession()) return null;
    const document: string | null = new FileSessionStore(this.stateDirectory).load();
    if (document === null || !bridge.matchesStoredSession()) return null;
    try {
      const binding: string = await sha256Hex(document);
      return bridge.matchesStoredSession() ? binding : null;
    } catch {
      return null;
    }
  }

  private bridge(): AccountCloudBridge {
    return new AccountCloudBridge(this.transport, new FileSessionStore(this.stateDirectory));
  }
}
