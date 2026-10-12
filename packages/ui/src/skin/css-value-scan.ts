/** Visits syntax outside CSS strings and escapes. The visitor may stop scanning by returning false; the result is false only for an unfinished comment when comments are skipped. */
export function scanCssValue(
  value: string,
  start: number,
  skipComments: boolean,
  visit: (char: string, index: number) => boolean | void,
): boolean {
  let quote = "";
  for (let index = start; index < value.length; index++) {
    const char = value[index];
    if (char === "\\") {
      index++;
      continue;
    }
    if (quote) {
      if (char === quote) quote = "";
      continue;
    }
    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }
    if (skipComments && char === "/" && value[index + 1] === "*") {
      const end = value.indexOf("*/", index + 2);
      if (end < 0) return false;
      index = end + 1;
      continue;
    }
    if (visit(char, index) === false) break;
  }
  return true;
}
