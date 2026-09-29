import type { AccountCommunityDestination } from "../account/account-page";

export type CommunityDestination = AccountCommunityDestination | "all";
export type CommunityDestinationCategory = "skin" | "dictionary" | "reply";
export type CommunityDestinationScope = "" | "mine" | "saved";

export interface CommunityDestinationView {
  category: CommunityDestinationCategory;
  scope: CommunityDestinationScope;
  initialMine: boolean;
}

/** Maps an account/community destination to the initial community gallery view. */
export function communityDestinationView(
  destination: CommunityDestination,
): CommunityDestinationView {
  const category =
    destination === "published-reply" || destination === "saved-reply"
      ? "reply"
      : destination === "published-dictionary" || destination === "saved-dictionary"
        ? "dictionary"
        : "skin";
  const scope =
    destination === "published-dictionary" || destination === "published-reply"
      ? "mine"
      : destination === "saved-dictionary" || destination === "saved-reply"
        ? "saved"
        : "";
  return {
    category,
    scope,
    initialMine: destination === "published-skins",
  };
}
