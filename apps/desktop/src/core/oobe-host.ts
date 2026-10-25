import type { OobeClient, OobeState } from "@msime/ui";

type Invoke = <T>(command: string) => Promise<T>;
interface NativeOobeState {
  resources_ready: boolean | null;
  system_enabled: boolean | null;
  system_current: boolean | null;
  preferences_saved: boolean | null;
  onboarding_seen: boolean | null;
}

function fact(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

export const unknownOobeState = (): OobeState => ({
  resourcesReady: null,
  systemEnabled: null,
  systemCurrent: null,
  preferencesSaved: null,
  onboardingSeen: null,
});

export function createDesktopOobeClient(
  platform: string | undefined,
  invoke: Invoke,
  openSetup: () => Promise<void>,
  prepareResources?: () => Promise<void>,
): OobeClient | undefined {
  if (platform !== "macos" && platform !== "linux") return undefined;
  let cached: OobeState | null = null;
  let pending: Promise<void> | undefined;
  const listeners = new Set<(state: OobeState) => void>();
  const refresh = () => {
    if (pending) return;
    pending = invoke<NativeOobeState>("desktop_oobe_status")
      .then(
        (value) => {
          cached = {
            resourcesReady: fact(value?.resources_ready),
            systemEnabled: fact(value?.system_enabled),
            systemCurrent: fact(value?.system_current),
            preferencesSaved: fact(value?.preferences_saved),
            onboardingSeen: fact(value?.onboarding_seen),
          };
        },
        () => {
          cached = unknownOobeState();
        },
      )
      .then(() => {
        for (const listener of listeners) listener(cached!);
      })
      .finally(() => {
        pending = undefined;
      });
  };
  return {
    read: () => cached,
    subscribe: (listener) => {
      listeners.add(listener);
      refresh();
      return () => {
        listeners.delete(listener);
      };
    },
    refresh,
    openSetup,
    prepareResources,
  };
}
