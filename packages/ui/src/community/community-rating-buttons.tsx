import { ActionButton } from "../core/action-button";
import * as style from "./community-style";

export interface CommunityRatingButtonsProps {
  description: string;
  disabled: boolean;
  onRate: (stars: number) => void;
}

export function CommunityRatingButtons({
  description,
  disabled,
  onRate,
}: CommunityRatingButtonsProps) {
  return (
    <div
      className={`${style.divided} [&>p]:mt-0 [&>p]:mb-2.5 [&>p]:text-xs [&>p]:text-secondary`}
      aria-label="我的评分"
    >
      <p>{description}</p>
      <div className="grid grid-cols-5 gap-1.5">
        {[1, 2, 3, 4, 5].map((stars) => (
          <ActionButton
            key={stars}
            action={() => onRate(stars)}
            className="secondary min-w-0 px-[5px]"
            disabled={disabled}
            ariaLabel={`评 ${stars} 星`}
            label={`${stars} 星`}
          />
        ))}
      </div>
    </div>
  );
}
