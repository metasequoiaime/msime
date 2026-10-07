import { utf8Length } from '../Utf8';
import { SchemeTraits } from '../SchemeTraits';

/** Numeric fields the Harmony host reads from a JSON Engine view. */
export interface EngineViewNumericFields {
  readonly editing_text: string;
  readonly caret_position: number;
  readonly page: number;
  readonly page_count: number;
  readonly generation: number;
  readonly scheme: number;
}

/** Rejects malformed JSON values before they can enter keyboard state or candidate layout. */
export class EngineViewValuePolicy {
  /** Native replies are untrusted JSON; arrays and null must not be treated as records. */
  static isObject(value: unknown): value is Record<string, Object> {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
  }

  static isGeneration(value: unknown): boolean {
    return EngineViewValuePolicy.nonNegativeInteger(value as number);
  }

  static isValid(view: EngineViewNumericFields | null | undefined): boolean {
    if (!EngineViewValuePolicy.isObject(view)) return false;
    if (typeof view.editing_text !== 'string') return false;
    if (!EngineViewValuePolicy.nonNegativeInteger(view.caret_position)
      || !EngineViewValuePolicy.nonNegativeInteger(view.page)
      || !EngineViewValuePolicy.nonNegativeInteger(view.page_count)
      || !EngineViewValuePolicy.nonNegativeInteger(view.generation)
      || !EngineViewValuePolicy.nonNegativeInteger(view.scheme)
      || view.scheme >= SchemeTraits.NAMES.length) {
      return false;
    }
    return view.caret_position <= utf8Length(view.editing_text)
      && (view.page_count === 0 ? view.page === 0 : view.page < view.page_count);
  }

  private static nonNegativeInteger(value: number): boolean {
    return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
  }
}
