/** 服务商凭据测试按钮显示的当前配置结果。 */
export interface CredentialTestState {
  signature: string;
  busy: boolean;
  ok?: boolean;
  message: string;
}
