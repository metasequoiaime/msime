import { utf8ByteLength } from "../core/text";

/**
 * The user's own candidate glosses, as the Engine reads them.
 *
 * Windows documents dropping a `custom_translations.txt` into the profile directory: one entry per
 * line, source and gloss separated by a Tab, `#` for a comment, and the last spelling of a source
 * wins. The Engine reads it on every host — `prepare_translation_sidecar` looks in the user data
 * directory before the resources — so the file is not a Windows feature, only a Windows-shaped way
 * of delivering it.
 *
 * A phone has no such directory a person can reach. Parsing it here lets a settings page take the
 * same file, say what is in it, and write it where the Engine will look: the same feature by the
 * only route the platform offers.
 *
 * These rules mirror `EnglishDictionary::load_custom_translations` deliberately. A line accepted
 * here that the Engine drops would be a line the page promised to apply and did not.
 */
export type CustomTranslationEntry = { source: string; gloss: string; chinese: boolean };
export type CustomTranslationReport = {
  entries: CustomTranslationEntry[];
  /** Lines that carried something but could not be read as an entry. */
  skipped: number;
};

const MAX_BYTES = 1024 * 1024;
const MAX_ENTRIES = 20000;

/** A source is Chinese when any character is non-ASCII, which is how the Engine picks direction. */
function isChineseSource(source: string): boolean {
  for (const character of source) if ((character.codePointAt(0) ?? 0) > 0x7f) return true;
  return false;
}

export function parseCustomTranslations(text: string): CustomTranslationReport {
  const body = text.startsWith("﻿") ? text.slice(1) : text;
  const seen = new Map<string, CustomTranslationEntry>();
  let skipped = 0;
  for (const rawLine of body.split(/\r\n|[\r\n]/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const tab = line.indexOf("\t");
    // A leading tab leaves no source and a trailing one no gloss; the Engine drops both.
    if (tab <= 0 || tab + 1 >= line.length) {
      skipped++;
      continue;
    }
    const source = line.slice(0, tab).trimEnd();
    const gloss = line.slice(tab + 1).trim();
    if (!source || !gloss) {
      skipped++;
      continue;
    }
    // Last wins, as the Engine's map assignment does.
    seen.set(source, { source, gloss, chinese: isChineseSource(source) });
    if (seen.size > MAX_ENTRIES) break;
  }
  return { entries: [...seen.values()], skipped };
}

/** Whether a document is small enough to send across a bridge and keep on a phone. */
export function customTranslationsWithinBounds(text: string): boolean {
  return utf8ByteLength(text) <= MAX_BYTES;
}

export const customTranslationsExample =
  "# 每行一条，用 Tab 分隔源词和译文\n你好\thello\n刚才\ta moment ago; just now\nserendipity\t意外发现珍奇事物的本领\n";
