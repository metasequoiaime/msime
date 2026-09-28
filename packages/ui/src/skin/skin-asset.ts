const MAX_SKIN_ASSET_BYTES = 8 * 1024 * 1024;

export function validSkinAssetBytes(value: unknown, requireNonEmpty = false): value is number[] {
  return (
    Array.isArray(value) &&
    (requireNonEmpty ? value.length > 0 : true) &&
    value.length <= MAX_SKIN_ASSET_BYTES &&
    value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
  );
}
