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
