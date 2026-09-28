export type Modifier = "Shift" | "Caps Lock" | "Ctrl" | "Alt" | "Win";

export function modifierPrefix(modifiers: Set<Modifier>) {
  return ["Ctrl", "Alt", "Win", "Shift"]
    .filter((value) => modifiers.has(value as Modifier))
    .join("+");
}

// Width ratios from Windows KeyboardPanel.cpp at 7fa6fb1a7862c5ca1541b9cb839d9bea3a06e2c6.
// Identify the space row by its content: Linux adds function and numpad rows.
export function keyboardKeyWeight(label: string, index: number, spaceRow: boolean) {
  if (spaceRow) return label === "Space" ? 6.7 : 1.25;
  if (label === "Backspace") return 1.9;
  if (label === "Tab") return 1.5;
  if (label === "\\") return 1.4;
  if (label === "Caps Lock") return 1.85;
  if (label === "Enter") return 2;
  if (label === "Shift") return index === 0 ? 2.35 : 2.15;
  return 1;
}

export function isImeCommitKey(virtualKey: number) {
  return (
    [0x20, 0x0d, 0x09, 0x08, 0x2e, 0x6a, 0x6b, 0x6d, 0x6e, 0x6f].includes(virtualKey) ||
    (virtualKey >= 0x30 && virtualKey <= 0x39) ||
    (virtualKey >= 0x60 && virtualKey <= 0x69)
  );
}
