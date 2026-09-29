export type PreviewKey = { label: string; weight: number };

const key = (label: string, weight = 1): PreviewKey => ({ label, weight });
const letters = (text: string) => [...text].map((label) => key(label));

export const desktopKeyboardRows: PreviewKey[][] = [
  [...letters("`1234567890-="), key("Backspace", 1.9)],
  [key("Tab", 1.5), ...letters("qwertyuiop[]"), key("\\", 1.4)],
  [key("Caps Lock", 1.85), ...letters("asdfghjkl;'"), key("Enter", 2)],
  [key("Shift", 2.35), ...letters("zxcvbnm,./"), key("Shift", 2.15)],
  [
    key("Ctrl", 1.25),
    key("Win", 1.25),
    key("Alt", 1.25),
    key("Space", 6.7),
    key("Alt", 1.25),
    key("Win", 1.25),
    key("Del", 1.25),
    key("Ctrl", 1.25),
  ],
];

export const touchKeyboardRows: PreviewKey[][] = [
  letters("qwertyuiop"),
  [key("", 0.5), ...letters("asdfghjkl"), key("", 0.5)],
  [key("⇧", 1.5), ...letters("zxcvbnm"), key("⌫", 1.5)],
  [key("符号", 1.5), key("中/英", 1.5), key("空格", 4.5), key("，"), key("↵", 1.5)],
];

export const actionKeyboardLabels = new Set(["Backspace", "Enter", "Shift", "Del", "⇧", "⌫", "↵"]);
