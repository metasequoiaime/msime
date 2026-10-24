import { useEffect, useState } from "react";
import { parseAccountNamePresentation } from "./account-name-presentation";
import type { AccountClient, AccountNamePresentation, AccountProfile } from "./account-model";
import type { AccountActionState } from "./account-operation";

type NameClient = Pick<AccountClient, "namePresentation" | "rename">;
type NameState = {
  client: NameClient;
  input: string;
  value: AccountNamePresentation | null;
  limits: AccountNamePresentation | null;
};

/** 昵称提示与保存复用共享纯函数；旧输入、旧客户端和卸载后的回复不能生效。 */
export function useAccountRename({
  client,
  name,
  mounted,
  clientGeneration,
  perform,
  onRenamed,
}: {
  client: NameClient;
  name: string;
  mounted: AccountActionState["mounted"];
  clientGeneration: AccountActionState["clientGeneration"];
  perform: AccountActionState["perform"];
  onRenamed: (profile: AccountProfile) => void;
}) {
  const [state, setState] = useState<NameState | null>(null);
  useEffect(() => {
    let active = true;
    Promise.resolve()
      .then(() => client.namePresentation(name))
      .then(parseAccountNamePresentation)
      .then((value) => {
        if (active) setState({ client, input: name, value, limits: value });
      })
      .catch(() => {
        if (active)
          setState((previous) => ({
            client,
            input: name,
            value: null,
            limits: previous?.client === client ? previous.limits : null,
          }));
      });
    return () => {
      active = false;
    };
  }, [client, name]);

  const current = state?.client === client && state.input === name ? state : null;
  const limits = state?.client === client ? state.limits : null;
  const normalizedName = current?.value?.normalized ?? name;
  const validName = current?.value?.valid === true;
  const rename = () =>
    perform(async () => {
      const generation = clientGeneration.current;
      const value = parseAccountNamePresentation(await client.namePresentation(name));
      if (!mounted.current || generation !== clientGeneration.current) return;
      if (!value.valid) throw { code: "account_invalid" };
      const updated = await client.rename(value.normalized);
      if (!mounted.current || generation !== clientGeneration.current) return;
      onRenamed(updated);
    });

  return {
    normalizedName,
    validName,
    rename,
    editorState: {
      maximumChars: limits?.maximum_chars,
      maximumUtf16Units: limits?.maximum_utf16_units,
      checking: current === null,
      unavailable: current !== null && current.value === null,
    },
  };
}
