import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";

export interface OnlineAssistantConfig {
  enabled: boolean;
  model: string;
  candidate_limit: number;
}

export interface OnlineQuery {
  generation: number;
  session_id: number;
  identity: string;
  cache_key: string;
  cloud_eligible: boolean;
  ai_eligible: boolean;
  cloud_candidates: boolean;
  ai_assistant?: OnlineAssistantConfig | null;
}

interface AiMessage {
  content?: string;
}

interface AiChoice {
  message?: AiMessage;
}

interface AiEnvelope {
  choices?: AiChoice[];
  error?: Object | null;
}

interface AiCandidate {
  text?: unknown;
}

interface AiContent {
  candidates?: unknown[];
}

/** Bounds and identifies asynchronous cloud/AI results before they return to Engine. */
export class OnlineCandidatePolicy {
  static readonly QUIET_INTERVAL_MS: number = 350;
  /** 云候选的连接时限和整体时限，与 client-core 的 `CONNECT_TIMEOUT_MS` / `REQUEST_TIMEOUT_MS` 相同，由 scripts/test-cloud-request-budget.py 核对。系统 http 模块的 readTimeout 对应 libcurl 的 `CURLOPT_TIMEOUT_MS`，限的是整个请求，不是读空闲。 */
  static readonly CLOUD_TIMEOUT_MS: number = 2_000;
  static readonly MAX_CLOUD_RESPONSE_BYTES: number = 256 * 1024;
  static readonly MAX_AI_RESPONSE_BYTES: number = 1024 * 1024;

  /** Whether a cloud reply is bounded before it is handed to the native parser. */
  static acceptsCloudBody(body: string | null | undefined): boolean {
    return (
      body !== null &&
      body !== undefined &&
      body.length > 0 &&
      utf8Length(body) <= OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES
    );
  }

  static signature(query: OnlineQuery): string {
    const assistant: OnlineAssistantConfig | null | undefined = query.ai_assistant;
    return (
      `${query.session_id}:${query.cache_key}:${query.identity}:` +
      `${query.cloud_candidates}:${assistant?.enabled === true ? JSON.stringify(assistant) : ""}`
    );
  }

  /** 判断失败请求是否仍可释放当前签名，让同一输入在下一次渲染时重试。 */
  static shouldReleaseAfterFailure(
    requestSignature: string,
    currentSignature: string,
    requestEpoch: number,
    currentEpoch: number,
    requestHandle: number,
    currentHandle: number,
  ): boolean {
    return (
      requestEpoch === currentEpoch &&
      requestHandle === currentHandle &&
      requestSignature.length > 0 &&
      requestSignature === currentSignature
    );
  }

  /** A replaced or vanished query must invalidate any timer or request for the old composition. */
  static hasActiveWork(
    signature: string,
    timerActive: boolean,
    requestCount: number,
  ): boolean {
    return signature.length > 0 || timerActive || requestCount > 0;
  }

  static aiCandidates(body: string, limit: number): string[] | null {
    if (
      utf8Length(body) > OnlineCandidatePolicy.MAX_AI_RESPONSE_BYTES ||
      !Number.isInteger(limit) ||
      limit < 1 ||
      limit > 10
    ) {
      return null;
    }
    try {
      const envelope: AiEnvelope = JSON.parse(body) as AiEnvelope;
      if (
        (envelope.error !== undefined && envelope.error !== null) ||
        envelope.choices === undefined ||
        envelope.choices.length === 0
      ) {
        return null;
      }
      const content: string | undefined = envelope.choices[0].message?.content;
      if (content === undefined || utf8Length(content) > 64 * 1024) return null;
      const document: AiContent = JSON.parse(content) as AiContent;
      if (document.candidates === undefined || !Array.isArray(document.candidates)) return null;
      const result: string[] = [];
      for (const candidate of document.candidates) {
        if (candidate === null || typeof candidate !== "object" || Array.isArray(candidate))
          continue;
        const text: unknown = (candidate as AiCandidate).text;
        if (
          typeof text !== "string" ||
          text.trim().length === 0 ||
          utf8Length(text) > 4096 ||
          TextPolicy.hasControl(text)
        ) {
          continue;
        }
        if (!result.includes(text)) result.push(text);
        if (result.length === limit) break;
      }
      return result;
    } catch {
      return null;
    }
  }
}
