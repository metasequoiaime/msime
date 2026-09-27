import { utf8Length } from "./Utf8";

/** Text validation shared by settings, network and candidate policies. */
export class TextPolicy {
  /** Accepts one URL scheme with a non-empty authority before callers apply their own bounds. */
  static hasAuthority(value: string, scheme: string): boolean {
    if (!value.startsWith(scheme)) return false;
    const authority: string = value.substring(scheme.length).split("/")[0].split("?")[0];
    return authority.length > 0;
  }

  /** Accepts an HTTPS URL with a non-empty authority before callers apply their own size rules. */
  static hasHttpsAuthority(value: string): boolean {
    return TextPolicy.hasAuthority(value, "https://");
  }

  /** Rejects the C0 and C1 control ranges while leaving printable Unicode untouched. */
  static hasControl(value: string): boolean {
    return Array.from(value).some((character: string): boolean => {
      const code: number = character.codePointAt(0) ?? 0;
      return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
    });
  }

  /** Bounds text while allowing the line breaks and tabs used in prompts and transcripts. */
  static validMultiline(value: string, maxBytes: number, requireNonEmpty: boolean): boolean {
    return (!requireNonEmpty || value.trim().length > 0) && utf8Length(value) <= maxBytes
      && !Array.from(value).some((character: string): boolean => {
        const code: number = character.codePointAt(0) ?? 0;
        return code < 0x20 && code !== 0x09 && code !== 0x0a && code !== 0x0d;
      });
  }
}
