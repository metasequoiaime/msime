/** 受限同步清除桥只接账号目录与已准备的运行时选项。 */
export interface DictionaryLearningResetRequest {
  directory: string;
  options: Object;
}

interface LearningResetAction {
  operation: string;
}

/** 转换为固定清除请求前保留旧词库入口的严格动作校验，不能悄悄丢弃额外字段。 */
export function validLearningResetAction(action: LearningResetAction): boolean {
  return action.operation === "reset" && Object.keys(action).length === 1;
}
