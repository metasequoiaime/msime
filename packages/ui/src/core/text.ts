/** Keep user-visible text within a grapheme limit without splitting emoji or combining marks. */
export function boundedGraphemes(value: string, maximum: number): string {
  if (typeof Intl !== "undefined" && "Segmenter" in Intl) {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
    return Array.from(segmenter.segment(value), (item) => item.segment)
      .slice(0, maximum)
      .join("");
  }
  return [...value].slice(0, maximum).join("");
}

const utf8Encoder = new TextEncoder();

/** Return the UTF-8 byte length used by bridge and engine text limits. */
export function utf8ByteLength(value: string): number {
  return utf8Encoder.encode(value).byteLength;
}
