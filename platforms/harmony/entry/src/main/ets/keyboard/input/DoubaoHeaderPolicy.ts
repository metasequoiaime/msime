import { TextPolicy } from "../TextPolicy";
import { utf8Length } from "../Utf8";

/** Validates values before they become native WebSocket handshake headers. */
export class DoubaoHeaderPolicy {
  static validValue(value: string, maxBytes: number): boolean {
    const trimmed: string = value.trim();
    return trimmed.length > 0 && utf8Length(trimmed) <= maxBytes && !TextPolicy.hasControl(trimmed)
      && Array.from(trimmed).every((character: string): boolean => {
        const code: number = character.charCodeAt(0);
        return code >= 0x20 && code <= 0x7e;
      });
  }

  static validConfiguration(
    authMode: string,
    token: string,
    resourceId: string,
    appKey: string,
  ): boolean {
    return (authMode === "api_key" || authMode === "legacy")
      && DoubaoHeaderPolicy.validValue(token, 8192)
      && DoubaoHeaderPolicy.validValue(resourceId, 256)
      && (authMode !== "legacy" || DoubaoHeaderPolicy.validValue(appKey, 8192));
  }
}
