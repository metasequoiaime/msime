export type ReleaseAsset = { name?: unknown; digest?: unknown };

const githubDigestPattern = /^sha256:([0-9a-f]{64})$/;

export function selectUniqueReleaseAsset(
  assets: unknown,
  patterns: readonly RegExp[],
): { name: string; sha256: string | null } | null {
  if (!Array.isArray(assets)) return null;
  for (const pattern of patterns) {
    const matches = assets.filter(
      (asset: ReleaseAsset | null): asset is ReleaseAsset & { name: string } =>
        !!asset &&
        typeof asset === "object" &&
        typeof asset.name === "string" &&
        pattern.test(asset.name),
    );
    if (matches.length === 0) continue;
    if (matches.length > 1) return null;
    const [asset] = matches;
    const digest = typeof asset.digest === "string" ? githubDigestPattern.exec(asset.digest) : null;
    return { name: asset.name, sha256: digest?.[1] ?? null };
  }
  return null;
}
