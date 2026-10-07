import { utf8Length } from '../keyboard/Utf8';

export const MAX_SESSION_BYTES: number = 64 * 1024;

export function sessionFitsStorage(value: string): boolean {
  return value.length > 0 && utf8Length(value) <= MAX_SESSION_BYTES;
}
