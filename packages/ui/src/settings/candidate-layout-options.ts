/** 候选项排列方式，供候选设置页和宿主投影共用。 */
export type CandidateLayout = "horizontal" | "vertical";

export const candidateLayoutOptions = [
  { value: "horizontal", label: "横向" },
  { value: "vertical", label: "纵向" },
] as const satisfies readonly { value: CandidateLayout; label: string }[];
