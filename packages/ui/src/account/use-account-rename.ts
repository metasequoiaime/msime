import { isValidAccountName, normalizeAccountName } from "./account-labels";
import type { AccountClient, AccountProfile } from "./account-model";
import type { AccountActionState } from "./account-operation";

/** Shares nickname validation, update, and stale-client protection across profile views. */
export function useAccountRename({
  client,
  name,
  mounted,
  clientGeneration,
  perform,
  onRenamed,
}: {
  client: Pick<AccountClient, "rename">;
  name: string;
  mounted: AccountActionState["mounted"];
  clientGeneration: AccountActionState["clientGeneration"];
  perform: AccountActionState["perform"];
  onRenamed: (profile: AccountProfile) => void;
}) {
  const normalizedName = normalizeAccountName(name);
  const validName = isValidAccountName(name);
  const rename = () =>
    perform(async () => {
      const generation = clientGeneration.current;
      if (!validName) throw { code: "account_invalid" };
      const updated = await client.rename(normalizedName);
      if (!mounted.current || generation !== clientGeneration.current) return;
      onRenamed(updated);
    });

  return { normalizedName, validName, rename };
}
