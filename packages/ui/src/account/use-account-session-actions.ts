import type { AccountClient, AccountDeletion } from "./account-model";
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

/** 注销成功了、但本机匿名账号名下的语音样本这次没有删掉（宿主返回 `voice_samples.ok === false`）。不返回结果的宿主不做这一步，算作没有留下。 */
export function accountDeletionKeptVoiceSamples(result: AccountDeletion | void): boolean {
  return !!result && result.voice_samples?.ok === false;
}

/** 注销成功、语音样本没删掉时给用户看的一句。 */
export const ACCOUNT_DELETED_VOICE_SAMPLES_KEPT =
  "账号已注销，但这台设备匿名账号名下的语音样本这次没能删除。";

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
  /** voiceSamplesKept 见 {@link accountDeletionKeptVoiceSamples}。 */
  onDeleted: (voiceSamplesKept: boolean) => void;
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
      const result = await client.deleteAccount();
      if (!mounted.current || generation !== clientGeneration.current) return;
      onDeleted(accountDeletionKeptVoiceSamples(result));
    });

  return { signOut, deleteAccount };
}
