/** 桌面宿主的粤拼解释方式；缺字段或未来版本的未知值按普通粤拼。触屏宿主不读这一项，粤语双拼在那里是独立的触屏方案。 */
export type CantonesePreferences = {
  input_method?: "jyutping" | "shuangpin";
};

export function resolveCantoneseInputMethod(preferences?: CantonesePreferences) {
  return preferences?.input_method === "shuangpin" ? "shuangpin" : "jyutping";
}

export const cantoneseJyutpingDescription =
  "按不带声调的粤拼输入，用 ' 分隔音节，数字键选词，候选为繁体字词";

export const cantoneseShuangpinDescription =
  "两键一个音节，第一键是声母、第二键是韵母，ing 和 ik 在 ; 键上；从第一键起按双拼解释，与粤拼同形的码（如 oi）也按双拼。数字键选词，候选为繁体字词";
