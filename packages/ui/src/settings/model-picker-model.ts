export interface ModelPickerModel {
  known: string[];
  listKey: string;
  custom: boolean;
}

/** 去重服务商预置和已获取的模型，并判断当前值是否需要自定义输入框。 */
export function modelPickerModel(models: readonly string[], model: string): ModelPickerModel {
  const known = [...new Set(models.filter((entry) => entry.length > 0))];
  const listKey = known.join("\n");
  return { known, listKey, custom: !known.includes(model) };
}
