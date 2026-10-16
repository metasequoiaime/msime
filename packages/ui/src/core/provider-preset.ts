/** 服务商预设的展示信息；实际请求仍使用用户保存的偏好。 */
export interface ProviderPreset {
  models?: readonly string[];
  documentation?: string;
}

/** 服务商的默认端点和模型，以及可选的展示信息。 */
export type ProviderDefaults = Pick<ProviderPreset, keyof ProviderPreset> & {
  endpoint: string;
  model: string;
};
