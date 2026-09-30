import { deepEqual, isPlainObject } from "../core/deep-equal";

/** One edited leaf of a preferences document; `value` is `undefined` when the edit removed the key. */
export interface PreferenceChange {
  path: readonly string[];
  value: unknown;
}

/**
 * The edits that turn `base` into `next`, one per changed leaf. Plain objects are walked key by key so two writers touching different fields of the same group do not collide; arrays and every other value are replaced whole.
 */
export function preferenceChanges(
  base: unknown,
  next: unknown,
  path: readonly string[] = [],
): PreferenceChange[] {
  if (deepEqual(base, next)) return [];
  if (!isPlainObject(base) || !isPlainObject(next)) return [{ path, value: next }];
  const keys = new Set([...Object.keys(base), ...Object.keys(next)]);
  return [...keys].flatMap((key) => preferenceChanges(base[key], next[key], [...path, key]));
}

function valueAt(value: unknown, path: readonly string[]): unknown {
  let current = value;
  for (const key of path) {
    if (!isPlainObject(current)) return undefined;
    current = current[key];
  }
  return current;
}

function withValueAt(target: unknown, path: readonly string[], value: unknown): unknown {
  if (path.length === 0) return value;
  const [key, ...rest] = path;
  const current = isPlainObject(target) ? target : {};
  const child = withValueAt(current[key], rest, value);
  const next = { ...current };
  if (child === undefined) delete next[key];
  else next[key] = child;
  return next;
}

/** Applies `changes` on top of `target` without mutating it. */
export function applyPreferenceChanges<T>(target: T, changes: readonly PreferenceChange[]): T {
  return changes.reduce<unknown>(
    (current, change) => withValueAt(current, change.path, change.value),
    target,
  ) as T;
}

function related(left: readonly string[], right: readonly string[]): boolean {
  const length = Math.min(left.length, right.length);
  for (let index = 0; index < length; index += 1) if (left[index] !== right[index]) return false;
  return true;
}

/**
 * Whether a local edit replaced something another writer also changed to a different value: the same leaf, or one path containing the other.
 */
export function preferenceChangesCollide(
  local: readonly PreferenceChange[],
  remote: readonly PreferenceChange[],
  latest: unknown,
): boolean {
  return local.some(
    (change) =>
      remote.some((other) => related(change.path, other.path)) &&
      !deepEqual(valueAt(latest, change.path), change.value),
  );
}
