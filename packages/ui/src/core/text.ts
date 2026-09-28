/** Keep at most `maximum` user-perceived characters, with a code-point fallback for old runtimes. */
export function truncateGraphemes(value: string, maximum: number): string {
  if (typeof Intl !== "undefined" && "Segmenter" in Intl) {
    const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });
    return Array.from(segmenter.segment(value), (item) => item.segment)
      .slice(0, maximum)
      .join("");
  }
  return [...value].slice(0, maximum).join("");
}
