import type { FormEvent } from "react";

export interface CreateSettingsSaveActionOptions {
  save: () => Promise<void>;
}

/** Creates the form submit handler that saves settings without navigating the page. */
export function createSettingsSaveAction({ save }: CreateSettingsSaveActionOptions) {
  return (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void save();
  };
}
