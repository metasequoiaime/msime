/**
 * Describes whether this HAP was built with the signed shared-sandbox capability.
 *
 * HarmonyOS does not let an application invent a data-group id: AppGallery Connect
 * issues it and the signing profile must carry it. Keep the value empty until a
 * profile with that capability is available, so callers can expose the real
 * fallback instead of claiming that settings and the input-method extension share
 * files/state.
 */
export type HarmonySharedSandboxStatus = {
  available: false;
  kind: "fallback";
  reason: "data-group-id-unconfigured";
};

export class HarmonySharedSandboxPolicy {
  private static readonly DATA_GROUP_ID: string = "";

  static status(): HarmonySharedSandboxStatus {
    return {
      available: false,
      kind: "fallback",
      reason: "data-group-id-unconfigured",
    };
  }
}
