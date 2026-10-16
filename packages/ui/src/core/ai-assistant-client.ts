export type AiAssistantClient = {
  // `provider` 供持有凭据的宿主服务核对私有配置。
  // 宿主自行持有令牌时忽略它，改用 `token` 认证。
  fetchModels(configuration: {
    endpoint: string;
    token: string;
    provider: string;
  }): Promise<string[]>;
  test(configuration: {
    endpoint: string;
    model: string;
    prompt: string;
    token: string;
    text: string;
    provider: string;
  }): Promise<string>;
};
