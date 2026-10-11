/** Chat composers grow with their contents, up to four 22px lines plus 16px padding. */
export function resizeChatComposer(field: HTMLTextAreaElement | null): void {
  if (!field) return;
  field.style.height = "auto";
  field.style.height = `${Math.min(field.scrollHeight, 22 * 4 + 16)}px`;
}
