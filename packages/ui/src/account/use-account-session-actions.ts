import type { AccountClient } from "./account-model";
import type { AccountActionState } from "./account-operation";

export type AccountSessionAction = "logout" | "logout-all" | "delete";

/** Dispatches the session choices used by each account profile layout. */
export function runAccountSessionAction(
  action: AccountSessionAction,
  { signOut, deleteAccount }: { signOut: (all: boolean) => void; deleteAccount: () => void },
) {
  if (action === "logout") signOut(false);
  else if (action === "logout-all") signOut(true);
  else if (action === "delete") deleteAccount();
}

/** Shares logout and account deletion while guarding callbacks against stale clients. */
export function useAccountSessionActions({
  client,
  mounted,
  clientGeneration,
  perform,
  onSignedOut,
  onDeleted,
}: {
  client: Pick<AccountClient, "logout" | "deleteAccount">;
  mounted: AccountActionState["mounted"];
  clientGeneration: AccountActionState["clientGeneration"];
  perform: AccountActionState["perform"];
  onSignedOut: (all: boolean) => void;
  onDeleted: () => void;
}) {
  const signOut = (all: boolean) =>
    perform(async () => {
      const generation = clientGeneration.current;
      await client.logout(all);
      if (!mounted.current || generation !== clientGeneration.current) return;
      onSignedOut(all);
    });

  const deleteAccount = () =>
    perform(async () => {
      const generation = clientGeneration.current;
      await client.deleteAccount();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onDeleted();
    });

  return { signOut, deleteAccount };
}
