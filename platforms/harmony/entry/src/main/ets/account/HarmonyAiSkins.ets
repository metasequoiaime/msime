import client from 'libmsimeclient.so';
import { AccountCloudBridge, accountReplyValue } from './AccountCloudBridge';
import { utf8Length } from '../keyboard/Utf8';
import {
  AiSkinCancelled,
  AiSkinFailure,
  AiSkinPlan,
  AiSkinProposal,
  AiSkinRun,
  AiSkinRunner,
  ArtworkJob
} from './AiSkinRunPolicy';

/**
 * AI skin generation on this host, which performs its own requests.
 *
 * `BackendAiSkinService::generate` runs the whole pipeline in Rust, and the hosts whose HTTP is
 * Rust's use it. This host's settings surface reaches the network through the system stack, so it
 * performs the four requests itself and asks the shared client for everything that is not the
 * transfer: what to say to the model, whether the answer is three usable designs, and whether a
 * returned picture is one this client will show. That is the same split `aiRequestForQuery` and
 * `parseAiResponse` already make for candidates.
 *
 * The sequencing lives in `AiSkinRunPolicy` and the requests live here. The rules between the
 * steps — cancel between every one, stop the siblings when one fails, release a job whichever way
 * it ends — are where this goes wrong, and none of them need a network to be tested.
 */

interface PlanReply {
  ok: boolean;
  value?: Object;
  error?: string;
}

/**
 * The three request shapes `msime_client_ai_skin_plan` accepts.
 *
 * One interface rather than three because ArkTS requires every object literal to name a declared
 * type, and the alternative — three near-identical interfaces — would put the difference between
 * them in their names rather than in the `operation` field that actually decides it.
 */
interface PlanRequest {
  operation: string;
  prompt?: string;
  model?: string;
  text?: string;
  artwork?: Object;
}

/** An accepted reply with nothing to say, which the page reads as success. */
interface EmptyValue {}

interface SuccessReply {
  ok: boolean;
  value: EmptyValue;
}

interface ComposedRequest {
  path?: string;
  body?: Object;
}

/** A completion is a model writing; the shared clients allow it the same 125 seconds. */
const CHAT_TIMEOUT_MS: number = 125 * 1000;

export class HarmonyAiSkins {
  private readonly bridge: AccountCloudBridge;
  private readonly runs: Map<string, AiSkinRun> = new Map<string, AiSkinRun>();

  constructor(bridge: AccountCloudBridge) {
    this.bridge = bridge;
  }

  /**
   * One generation, named by the request id the page chose.
   *
   * The id is the page's handle on a run it may want to stop, so a second request under an id
   * already running is refused rather than silently started: two runs answering to one name would
   * leave the cancel button pointing at whichever finished last.
   */
  async generate(requestId: string, prompt: string,
    progress: (completed: number) => void): Promise<string> {
    if (!HarmonyAiSkins.validRequestId(requestId)) {
      return HarmonyAiSkins.failure('ai_skin_invalid');
    }
    if (this.runs.has(requestId)) return HarmonyAiSkins.failure('ai_skin_busy');
    const run = new AiSkinRun(this.runner(requestId));
    this.runs.set(requestId, run);
    try {
      const proposals: AiSkinProposal[] = await run.generate(prompt, progress);
      return JSON.stringify({ ok: true, value: proposals });
    } catch (error) {
      if (error instanceof AiSkinCancelled) return HarmonyAiSkins.failure('ai_skin_cancelled');
      if (error instanceof AiSkinFailure) return HarmonyAiSkins.failure(error.message);
      return HarmonyAiSkins.failure('ai_skin_unavailable');
    } finally {
      this.runs.delete(requestId);
    }
  }

  /**
   * Stops a run.
   *
   * Cancelling something that is not running is not an error. The page sends this when it leaves
   * the screen, and by then the run may have finished on its own a moment earlier.
   */
  cancel(requestId: string): string {
    if (!HarmonyAiSkins.validRequestId(requestId)) {
      return HarmonyAiSkins.failure('ai_skin_invalid');
    }
    this.runs.get(requestId)?.cancel();
    this.bridge.cancelAiSkinRequests(requestId);
    return JSON.stringify({ ok: true, value: {} as EmptyValue } as SuccessReply);
  }

  /** The page's own handle, bounded so it cannot become a key of arbitrary size. */
  private static validRequestId(value: string): boolean {
    return typeof value === 'string' && value.length > 0 && utf8Length(value) <= 96
      && /^[0-9a-zA-Z_-]+$/.test(value);
  }

  private static failure(code: string): string {
    return JSON.stringify({ ok: false, error: code });
  }

  private plan(request: PlanRequest): Object | null {
    try {
      const reply: PlanReply = JSON.parse(client.aiSkinPlan(JSON.stringify(request))) as PlanReply;
      return accountReplyValue(reply);
    } catch {
      return null;
    }
  }

  private runner(requestTag: string): AiSkinRunner {
    const bridge = this.bridge;
    const owner = this;
    return {
      defaultModel: async (): Promise<string> => {
        const reply = await bridge.aiSkinRequest('GET', '/v1/models', undefined, undefined, requestTag);
        if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
          throw new AiSkinFailure(reply.error ?? 'ai_skin_unavailable');
        }
        const model: Object | undefined = (reply.value as Record<string, Object>)['default_model'];
        if (typeof model !== 'string' || model.length === 0 || utf8Length(model) > 200) {
          throw new AiSkinFailure('ai_skin_response');
        }
        return model;
      },
      chat: async (prompt: string, model: string): Promise<string> => {
        // The instruction is the shared one. A host that wrote its own would be asking for a
        // document the shared parser was not written against.
        const composed: Object | null =
          owner.plan({ operation: 'compose', prompt: prompt, model: model } as PlanRequest);
        if (composed === null) throw new AiSkinFailure('ai_skin_invalid');
        const request = composed as ComposedRequest;
        if (typeof request.path !== 'string' || request.body === undefined) {
          throw new AiSkinFailure('ai_skin_invalid');
        }
        const reply = await bridge.aiSkinRequest('POST', request.path,
          request.body as Record<string, Object>, CHAT_TIMEOUT_MS, requestTag);
        if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
          throw new AiSkinFailure(reply.error ?? 'ai_skin_unavailable');
        }
        const choices: Object | undefined = (reply.value as Record<string, Object>)['choices'];
        if (!Array.isArray(choices) || choices.length === 0) {
          throw new AiSkinFailure('ai_skin_response');
        }
        const first = choices[0] as Record<string, Object>;
        const message = first['message'] as Record<string, Object> | undefined;
        const content = message === undefined ? undefined : message['content'];
        if (message === undefined || message['role'] !== 'assistant' || typeof content !== 'string'
          || content.trim().length === 0) {
          throw new AiSkinFailure('ai_skin_response');
        }
        return content;
      },
      plans: (text: string): AiSkinPlan[] => {
        const parsed: Object | null = owner.plan({ operation: 'parse', text: text } as PlanRequest);
        if (parsed === null || !Array.isArray(parsed)) throw new AiSkinFailure('ai_skin_response');
        return parsed as AiSkinPlan[];
      },
      createJob: async (artworkPrompt: string): Promise<ArtworkJob> => {
        const reply = await bridge.aiSkinRequest('POST', '/v1/skins/jobs',
          { prompt: artworkPrompt }, undefined, requestTag);
        if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
          throw new AiSkinFailure(reply.error ?? 'ai_skin_unavailable');
        }
        return reply.value as ArtworkJob;
      },
      readJob: async (id: string): Promise<ArtworkJob> => {
        const reply = await bridge.aiSkinRequest('GET', `/v1/skins/jobs/${id}`, undefined, undefined, requestTag);
        if (reply.error !== undefined || reply.value === undefined || reply.value === null) {
          throw new AiSkinFailure(reply.error ?? 'ai_skin_unavailable');
        }
        return reply.value as ArtworkJob;
      },
      deleteJob: async (id: string): Promise<void> => {
        await bridge.aiSkinRequest('DELETE', `/v1/skins/jobs/${id}`, undefined, undefined, requestTag);
      },
      validateArtwork: (artwork: Object): boolean =>
        owner.plan({ operation: 'artwork', artwork: artwork } as PlanRequest) !== null,
      wait: (milliseconds: number): Promise<void> =>
        new Promise<void>((resolve: () => void): void => {
          setTimeout(resolve, milliseconds);
        }),
      now: (): number => Date.now()
    };
  }
}
