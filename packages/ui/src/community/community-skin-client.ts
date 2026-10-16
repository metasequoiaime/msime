import type {
  CommunitySkin,
  CommunitySkinClient,
  CommunitySkinDownload,
  CommunitySkinPage,
} from "./community-skin-model";

export type CommunitySkinOperation =
  | "list"
  | "detail"
  | "download"
  | "rate"
  | "publish"
  | "unpublish"
  | "set_category"
  | "finish_trial"
  | "report";

export type CommunitySkinRequest = <T>(
  operation: CommunitySkinOperation,
  args: Record<string, unknown>,
) => Promise<T>;

/** 创建社区皮肤的共享操作客户端；不同宿主只负责把操作映射到自己的传输协议。 */
export function createCommunitySkinClient(request: CommunitySkinRequest): CommunitySkinClient {
  return {
    list: (offset, search, mine, category) =>
      request<CommunitySkinPage>("list", { offset, search, mine, category }),
    detail: (id) => request<CommunitySkin>("detail", { id }),
    download: (id, name) => request<CommunitySkinDownload>("download", { id, name }),
    rate: (id, stars) => request<void>("rate", { id, stars }),
    publish: (id, name, description, design, category) =>
      request<void>("publish", { id, name, description, design, category }),
    unpublish: (id) => request<void>("unpublish", { id }),
    setCategory: (id, category) => request<CommunitySkin>("set_category", { id, category }),
    finishTrial: (id, keep) => request<void>("finish_trial", { id, keep }),
    report: (id, reason, detail) => request<void>("report", { id, reason, detail }),
  };
}
