/** Formats a byte bound as whole mebibytes when possible, otherwise kilobytes. */
export function dictionaryFileSizeLabel(bytes: number): string {
  return bytes >= 1_048_576 && bytes % 1_048_576 === 0
    ? `${bytes / 1_048_576} MB`
    : `${Math.floor(bytes / 1024)} KB`;
}
