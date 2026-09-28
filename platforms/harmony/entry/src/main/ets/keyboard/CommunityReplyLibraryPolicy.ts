import { utf8Length } from './Utf8';
import { TextPolicy } from './TextPolicy';

export interface CommunityReplyTemplate {
  readonly id: string;
  readonly name: string;
  readonly prompt: string;
}

/** Validates the app-private CommunityLibrary.json shared by the settings app and IME. */
export class CommunityReplyLibraryPolicy {
  static readonly MAX_BYTES: number = 4 * 1024 * 1024;
  static readonly MAX_ITEMS: number = 50;
  static readonly MAX_FIELD_BYTES: number = 64 * 1024;

  static parse(document: string): CommunityReplyTemplate[] {
    if (utf8Length(document) > CommunityReplyLibraryPolicy.MAX_BYTES) return [];
    let decoded: Object;
    try {
      decoded = JSON.parse(document) as Object;
    } catch {
      return [];
    }
    if (!Array.isArray(decoded) || decoded.length > CommunityReplyLibraryPolicy.MAX_ITEMS) return [];
    const result: CommunityReplyTemplate[] = [];
    const ids: string[] = [];
    for (const value of decoded) {
      if (value === null || typeof value !== 'object') return [];
      const item: Record<string, Object> = value as Record<string, Object>;
      const content: Record<string, Object> | null = item.content !== undefined
        && item.content !== null && typeof item.content === 'object'
        ? item.content as Record<string, Object> : null;
      const id: string = typeof item.id === 'string' ? item.id : '';
      const kind: string = typeof item.kind === 'string' ? item.kind : '';
      if (kind !== 'reply') continue;
      const name: string = typeof item.name === 'string' ? item.name : '';
      const prompt: string = content !== null && typeof content.prompt === 'string'
        ? content.prompt : '';
      if (id.trim().length === 0 || name.trim().length === 0
        || prompt.trim().length === 0 || ids.includes(id)
        || utf8Length(id) > CommunityReplyLibraryPolicy.MAX_FIELD_BYTES
        || utf8Length(name) > CommunityReplyLibraryPolicy.MAX_FIELD_BYTES
        || utf8Length(prompt) > CommunityReplyLibraryPolicy.MAX_FIELD_BYTES
        || TextPolicy.hasControl(id)
        || TextPolicy.hasControl(name)
        || TextPolicy.hasControl(prompt)) return [];
      ids.push(id);
      result.push({ id: id, name: name, prompt: prompt });
    }
    return result;
  }
}
