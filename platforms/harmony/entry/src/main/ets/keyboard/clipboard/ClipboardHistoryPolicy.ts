/**
 * Privacy and size bounds for text-only clipboard history entries, ported from
 * platforms/android/java/app/msime/android/ClipboardHistoryPolicy.java.
 *
 * The byte bound is measured in UTF-8, which is what the store writes.
 */
import { utf8Length } from "../Utf8";

const MAX_CHARS: number = 10000;
const MAX_BYTES: number = 40000;

/** Count Unicode code points without treating an astral character as two characters. */
function codePointCount(text: string): number {
  let count: number = 0;
  for (const character of text) {
    count++;
  }
  return count;
}

export class ClipboardHistoryPolicy {
  static readonly LIMIT: number = 50;
  static readonly MAX_CHARS: number = MAX_CHARS;
  static readonly MAX_BYTES: number = MAX_BYTES;

  static acceptable(text: string | null): boolean {
    return (
      text !== null &&
      text.trim() !== "" &&
      !text.includes("\u0000") &&
      codePointCount(text) <= MAX_CHARS &&
      utf8Length(text) <= MAX_BYTES
    );
  }
}
