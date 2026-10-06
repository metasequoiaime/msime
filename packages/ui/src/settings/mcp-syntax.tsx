/** What a piece of a shown command or configuration is, which decides its colour. */
export type SyntaxKind =
  | "plain"
  | "program"
  | "flag"
  | "string"
  | "placeholder"
  | "key"
  | "punctuation";

export type SyntaxToken = { text: string; kind: SyntaxKind };

/** The text the tokens spell, which is also what the copy buttons put on the clipboard. */
export function tokensText(tokens: readonly SyntaxToken[]): string {
  return tokens.map((token) => token.text).join("");
}

export const plain = (text: string): SyntaxToken => ({ text, kind: "plain" });

const kindClass: Record<Exclude<SyntaxKind, "plain">, string> = {
  program: "text-syntax-program",
  flag: "text-syntax-flag",
  string: "text-syntax-string",
  placeholder: "text-syntax-placeholder italic",
  key: "text-syntax-key",
  punctuation: "text-muted",
};

/** A `<pre>` whose tokens are coloured by kind; its text content is exactly `tokensText(tokens)`. */
export function SyntaxBlock({
  tokens,
  className,
  "aria-label": ariaLabel,
}: {
  tokens: readonly SyntaxToken[];
  className: string;
  "aria-label": string;
}) {
  return (
    <pre className={className} aria-label={ariaLabel}>
      {tokens.map((token, index) =>
        token.kind === "plain" ? (
          token.text
        ) : (
          <span key={index} className={kindClass[token.kind]}>
            {token.text}
          </span>
        ),
      )}
    </pre>
  );
}

/** The tokens of `JSON.stringify(value, null, 2)`: object keys, string values and the punctuation between them, so that joining them gives that exact text. */
export function jsonTokens(value: unknown, indent = ""): SyntaxToken[] {
  const inner = `${indent}  `;
  if (Array.isArray(value)) {
    if (value.length === 0) return [{ text: "[]", kind: "punctuation" }];
    return [
      { text: "[", kind: "punctuation" },
      ...value.flatMap((item, index) => [
        plain(`\n${inner}`),
        ...jsonTokens(item, inner),
        ...(index < value.length - 1 ? [{ text: ",", kind: "punctuation" } as const] : []),
      ]),
      plain(`\n${indent}`),
      { text: "]", kind: "punctuation" },
    ];
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) return [{ text: "{}", kind: "punctuation" }];
    return [
      { text: "{", kind: "punctuation" },
      ...entries.flatMap(([key, item], index) => [
        plain(`\n${inner}`),
        { text: JSON.stringify(key), kind: "key" } as const,
        { text: ":", kind: "punctuation" } as const,
        plain(" "),
        ...jsonTokens(item, inner),
        ...(index < entries.length - 1 ? [{ text: ",", kind: "punctuation" } as const] : []),
      ]),
      plain(`\n${indent}`),
      { text: "}", kind: "punctuation" },
    ];
  }
  if (typeof value === "string") return [{ text: JSON.stringify(value), kind: "string" }];
  return [plain(JSON.stringify(value))];
}
