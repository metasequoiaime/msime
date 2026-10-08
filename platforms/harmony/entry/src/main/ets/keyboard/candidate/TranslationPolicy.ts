import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";

export interface TranslationCandidate {
  text: string;
}

export interface TranslationProviderConfig {
  enabled: boolean;
  endpoint: string;
  api_key: string;
}

export interface TencentTranslationConfig {
  enabled: boolean;
  secret_id: string;
  secret_key: string;
  region: string;
}

export interface NiuTransTranslationConfig {
  enabled: boolean;
  app_id: string;
  apikey: string;
}

export interface TranslationQuery {
  generation: number;
  target_language: string;
  target_languages?: string[];
  provider?: string;
  translation_account?: boolean;
  candidates: TranslationCandidate[];
  custom_translation?: TranslationProviderConfig | null;
  tencent_tmt?: TencentTranslationConfig | null;
  niutrans?: NiuTransTranslationConfig | null;
  english_gloss: boolean;
  /** True only for the `/fy` command's request: one English sentence for the selected service, into target_language. */
  sentence?: boolean;
  resources?: string;
  user_data?: string;
  /** Non-English targets with an offline dictionary installed beside the resources; omitted when there are none. */
  offline_gloss_languages?: string[];
}

export interface TranslationPlanItem {
  text: string;
  key: string;
  source_language: string;
  target_language: string;
}

export interface TranslationEntry {
  text: string;
  translation: string;
}

/** Shared bounds and provider precedence for Harmony candidate translations. */
export class TranslationPolicy {
  static readonly QUIET_INTERVAL_MS: number = 500;
  static readonly MAX_QUERY_BYTES: number = 64 * 1024;
  static readonly MAX_RESPONSE_BYTES: number = 1024 * 1024;
  static readonly MAX_TRANSLATION_BYTES: number = 4096;
  static readonly MAX_CACHE_ENTRIES: number = 4096;
  static readonly NEGATIVE_CACHE_MS: number = 8 * 60 * 1000;

  static targets(query: TranslationQuery): string[] {
    const result: string[] = [];
    const values: string[] = [query.target_language];
    if (query.target_languages !== undefined) {
      for (const value of query.target_languages) values.push(value);
    }
    for (const value of values) {
      if (typeof value !== "string" || value.length === 0 || result.includes(value)) continue;
      result.push(value);
    }
    return result;
  }

  /** The targets answered from an installed offline dictionary, in target order. English keeps its own gloss path. */
  static offlineTargets(query: TranslationQuery): string[] {
    const installed: string[] = query.offline_gloss_languages ?? [];
    return TranslationPolicy.targets(query).filter(
      (target: string): boolean => target !== "en" && installed.includes(target),
    );
  }

  /** Adds one target's offline glosses for the candidates its online provider left unanswered: the user's own translator outranks the dictionary, which fills the rest. */
  static fill(answered: TranslationEntry[], offline: TranslationEntry[]): void {
    for (const entry of offline) {
      if (answered.some((existing: TranslationEntry): boolean => existing.text === entry.text))
        continue;
      TranslationPolicy.append(answered, entry.text, entry.translation);
    }
  }

  /** The one item a `/fy` request (command mode) asks about, or null for a candidate gloss query. The shared query sends it whatever the gloss switches say, with one candidate, the English typed after the command, and its own target language (Chinese). The candidate plan cannot carry it - it refuses a Chinese target and judges single words, not sentences - so it is asked as this item directly, with no offline dictionary and no translation cache, and its answer becomes the command's first row. */
  static commandItem(query: TranslationQuery): TranslationPlanItem | null {
    if (query.sentence !== true || query.candidates.length !== 1) return null;
    const text: string = query.candidates[0].text;
    if (typeof text !== "string" || text.length === 0) return null;
    if (typeof query.target_language !== "string" || query.target_language.length === 0)
      return null;
    return { text: text, key: text, source_language: "en", target_language: query.target_language };
  }

  static provider(query: TranslationQuery): string {
    if (query.niutrans?.enabled === true) return "niutrans";
    if (query.custom_translation?.enabled === true) return "custom";
    if (query.tencent_tmt?.enabled === true) return "tencent";
    return "";
  }

  /** 签名不携带凭据原文，但凭据轮换仍必须让旧请求失效。 */
  static signature(query: TranslationQuery): string {
    const custom: TranslationProviderConfig | null = query.custom_translation ?? null;
    const niutrans: NiuTransTranslationConfig | null = query.niutrans ?? null;
    const tencent: TencentTranslationConfig | null = query.tencent_tmt ?? null;
    return JSON.stringify({
      generation: query.generation,
      target_languages: TranslationPolicy.targets(query),
      candidates: query.candidates.map((candidate: TranslationCandidate): string => candidate.text),
      english_gloss: query.english_gloss,
      provider: TranslationPolicy.provider(query),
      translation_account: query.translation_account === true,
      endpoint: custom?.endpoint ?? "",
      app_id: niutrans?.app_id ?? "",
      region: tencent?.region ?? "",
      custom_credential: TranslationPolicy.credentialFingerprint(custom?.api_key ?? ""),
      niutrans_credential: TranslationPolicy.credentialFingerprint(niutrans?.apikey ?? ""),
      tencent_id: TranslationPolicy.credentialFingerprint(tencent?.secret_id ?? ""),
      tencent_credential: TranslationPolicy.credentialFingerprint(tencent?.secret_key ?? ""),
    });
  }

  /** 为失效签名和缓存作用域生成不含凭据原文的稳定指纹。 */
  static credentialFingerprint(value: string): string {
    let hash: number = 2166136261;
    for (let index = 0; index < value.length; index++) {
      hash ^= value.charCodeAt(index);
      hash = Math.imul(hash, 16777619);
    }
    return `${value.length}:${hash >>> 0}`;
  }

  /** 判断失败请求是否仍可释放当前签名，让同一候选页在下一次刷新时重试。 */
  static shouldReleaseAfterFailure(requestSignature: string, currentSignature: string,
    requestEpoch: number, currentEpoch: number, requestHandle: number,
    currentHandle: number): boolean {
    return requestEpoch === currentEpoch && requestHandle === currentHandle
      && requestSignature.length > 0 && requestSignature === currentSignature;
  }

  /** A replaced or vanished query must invalidate any timer or request for the old candidate page. */
  static hasActiveWork(
    signature: string,
    timerActive: boolean,
    requestCount: number,
  ): boolean {
    return signature.length > 0 || timerActive || requestCount > 0;
  }

  /** 缓存达到容量时清空，避免长时间输入让键盘进程无限保留候选词。 */
  static shouldResetCache(size: number): boolean {
    return size >= TranslationPolicy.MAX_CACHE_ENTRIES;
  }

  /** 在线 provider 未完成时，即使离线词典有可应用条目，也必须允许相同候选页重试。 */
  static shouldReleaseAfterProviderFailure(provider: string, providerComplete: boolean,
    hasEntries: boolean): boolean {
    return provider.length > 0 && !providerComplete && hasEntries;
  }

  static providerScope(query: TranslationQuery): string {
    const provider: string = TranslationPolicy.provider(query);
    if (provider === "custom") {
      return `custom:${query.custom_translation?.endpoint ?? ""}:${TranslationPolicy.credentialFingerprint(
        query.custom_translation?.api_key ?? "")}`;
    }
    if (provider === "niutrans") {
      return `niutrans:${query.niutrans?.app_id ?? ""}:${TranslationPolicy.credentialFingerprint(
        query.niutrans?.apikey ?? "")}`;
    }
    if (provider === "tencent") {
      return `tencent:${query.tencent_tmt?.region ?? ""}:${TranslationPolicy.credentialFingerprint(
        query.tencent_tmt?.secret_id ?? "")}:${TranslationPolicy.credentialFingerprint(
        query.tencent_tmt?.secret_key ?? "")}`;
    }
    return provider;
  }

  static cacheKey(query: TranslationQuery, target: string, item: TranslationPlanItem): string {
    return JSON.stringify({
      provider: TranslationPolicy.providerScope(query),
      target: target,
      key: item.key,
      direction: `${item.source_language}>${item.target_language}`,
      source: item.source_language,
      itemTarget: item.target_language,
    });
  }

  static planRequest(target: string, candidates: TranslationCandidate[]): string {
    return JSON.stringify({
      target_language: target,
      candidates: candidates.map((candidate: TranslationCandidate): Object => ({
        text: candidate.text,
        source: 0,
      })),
    });
  }

  static append(entries: TranslationEntry[], text: string, translation: string): void {
    const value: string = TranslationPolicy.normalise(translation);
    if (value.length === 0) return;
    const existing: TranslationEntry | undefined = entries.find(
      (entry: TranslationEntry): boolean => entry.text === text,
    );
    if (existing === undefined) {
      entries.push({ text: text, translation: value });
      return;
    }
    if (existing.translation === value || existing.translation.includes(` / ${value}`)) return;
    const combined: string = `${existing.translation} / ${value}`;
    if (utf8Length(combined) <= TranslationPolicy.MAX_TRANSLATION_BYTES) {
      existing.translation = combined;
    }
  }

  static normalise(value: string): string {
    if (typeof value !== "string") return "";
    const trimmed: string = value.trim();
    if (
      trimmed.length === 0 ||
      utf8Length(trimmed) > TranslationPolicy.MAX_TRANSLATION_BYTES ||
      TextPolicy.hasControl(trimmed)
    )
      return "";
    return trimmed;
  }
}
