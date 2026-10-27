# Agent Note: Android 文本编辑面板选择当前行

Status: implemented

## Problem

#9767：文本编辑面板只有字符移动、全选和剪切复制等动作，编辑备忘录或文档时不能一键选中光标所在行。

## Decision

新增可排序的 `SELECT_LINE` 侧键，扩展共享偏好为八个侧键。面板从编辑器请求完整文本和当前选区，以活动端计算当前行的起止位置，再调用 `InputConnection.setSelection`；换行符不包含在选区内，光标正位于换行符时选择前一行。网格增加一行承载第八个侧键，保留固定方向键、文档首尾和换行键的位置。

共享偏好读入 `touch_text_edit_button_order` 时补上文档里没有的侧键（`deserialize_text_edit_button_order`）：选择行之前保存的偏好、设置导出和同步数据只列七个动作，缺的按默认排列接在末尾，用户排过的顺序不动。读盘时整份偏好要通过 `validate`，不补的话旧文档全部读不进来，Windows 的 Server 因此在 `host_prepare` 失败退出（Win7 正式包装在旧测试数据目录上实测复现）。重复或未知的动作仍然拒绝；直接提交的偏好结构体不经过这一步，缺项照旧按无效拒绝。

## Alternatives considered

- **发送带 Shift 的方向键模拟选择行**：不同编辑器对 Home、End、Shift 和换行边界的处理不一致，无法稳定选择同一范围。
- **只调用编辑器上下文菜单的“选择行”**：Android 没有跨编辑器统一的选择行菜单动作，WebView 等输入框也常常不实现该动作。

- **校验改为接受缺项**：同样能读进旧文档，但设置页提交残缺排列也会被放过，宿主界面的错误不再暴露；只在反序列化时补齐，结构体提交仍严格校验。

## Consequences

选择行依赖编辑器提供完整 `ExtractedText`；密码框、部分 WebView 或只返回部分文本的输入框会显示不支持提示，不会猜测范围。面板高度增加一行，八个侧键均可通过现有顺序设置移动。

## Verification

Android host 冒烟（含文本编辑模型和行范围边界）及 `git diff --check` 已通过；待运行 quick 门禁、笔记校验和 Android 模拟器触摸验收。
