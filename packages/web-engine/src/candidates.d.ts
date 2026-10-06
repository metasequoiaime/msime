// `@msime/web-engine/candidates.js` 的类型：候选栏模块的导出与包入口里的同名声明是同一份，这里只转出，不另写一套。只要候选栏、引擎由自己接管的页面（例如 TapTapGo）从这个入口导入，打包器就不会顺着入口的 `new Worker(new URL("./worker.js", import.meta.url))` 把包自带的 Worker 一起打进去。
export { CANDIDATE_BAR_TAG, PARTS, createCandidateBar } from "./index.js";
export type {
  CandidateBar,
  CandidateBarOptions,
  MsimeFrame,
  MsimeSkin,
  MsimeSkinPalette,
  MsimeSkinId,
  MsimeSkinLayout,
  MsimeSkinMode,
} from "./index.js";
