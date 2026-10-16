import type { AccountClient, AccountProviders } from "./account-model";
import type { AccountActionState } from "./account-operation";

/** Runs account recovery calls and ignores results from an old client or screen. */
export function useAccountRecoveryActions({
  client,
  mounted,
  clientGeneration,
  perform,
  onProvidersChange,
  onClearedExpired,
}: {
  client: Pick<AccountClient, "providers" | "clearExpired">;
  mounted: AccountActionState["mounted"];
  clientGeneration: AccountActionState["clientGeneration"];
  perform: AccountActionState["perform"];
  onProvidersChange?: (providers: AccountProviders) => void;
  onClearedExpired: () => void;
}) {
  const refreshProviders = () =>
    perform(async () => {
      const generation = clientGeneration.current;
      const value = await client.providers();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onProvidersChange?.(value);
    });

  const clearExpired = () =>
    perform(async () => {
      const generation = clientGeneration.current;
      await client.clearExpired();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onClearedExpired();
    });

  return { refreshProviders, clearExpired };
}
