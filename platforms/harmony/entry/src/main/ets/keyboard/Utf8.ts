/**
 * UTF-8 byte length, computed directly rather than through TextEncoder so the bound holds wherever
 * this runs. Java reaches for String.getBytes(UTF_8).length; this is the same measurement.
 */
export function utf8Length(text: string): number {
  let bytes: number = 0;
  for (let offset: number = 0; offset < text.length; offset++) {
    const code: number = text.charCodeAt(offset);
    if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (code >= 0xd800 && code <= 0xdbff && offset + 1 < text.length) {
      const low: number = text.charCodeAt(offset + 1);
      if (low >= 0xdc00 && low <= 0xdfff) {
        bytes += 4;
        offset++;
        continue;
      }
      bytes += 3;
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

export function utf8WriteComplete(text: string, written: number): boolean {
  return written === utf8Length(text);
}
