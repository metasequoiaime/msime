import type { KeyboardEvent } from "react";
import { boundedGraphemes } from "../core/text";

export interface CommunityPublishFields {
  normalizedName: string;
  normalizedDescription: string;
  nameValid: boolean;
  descriptionValid: boolean;
}

/** Normalizes and validates the shared name and description limits for community publishing. */
export function communityPublishFields(name: string, description: string): CommunityPublishFields {
  const normalizedName = name.trim();
  const normalizedDescription = description.trim();
  return {
    normalizedName,
    normalizedDescription,
    nameValid:
      normalizedName.length > 0 &&
      boundedGraphemes(normalizedName, 32) === normalizedName &&
      [...normalizedName].length <= 32,
    descriptionValid: [...normalizedDescription].length <= 280,
  };
}

/** Submits a community publish dialog from a text input without hijacking checkbox Enter keys. */
export function handleCommunityPublishKeyDown(
  event: KeyboardEvent<HTMLElement>,
  submit: () => void,
) {
  if (event.key !== "Enter" || !(event.target instanceof HTMLInputElement)) return;
  event.preventDefault();
  if (event.target.type !== "checkbox") submit();
}
