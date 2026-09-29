export type TokenMap = Record<string, string>;

export function known(
  table: Record<string, { endpoint: string; model: string }>,
  field: "endpoint" | "model",
): string[] {
  return Object.values(table)
    .map((entry) => entry[field])
    .filter(Boolean);
}

export function fillIfDefault(
  current: string | undefined,
  next: string,
  defaults: string[],
): string | undefined {
  const value = (current ?? "").trim();
  if (value && !defaults.includes(value)) return undefined;
  return next;
}

export function swapTokenSlot(
  from: string,
  to: string,
  box: string,
  slots: TokenMap | undefined,
): { tokens: TokenMap; token: string } {
  const next: TokenMap = { ...slots };
  if (from) {
    if (box) next[from] = box;
    else delete next[from];
  }
  return { tokens: next, token: next[to] ?? "" };
}
