# Agent Note: iOS 14/18 键字母键宽度

Status: implemented

## Problem

iOS 的 14 键和 18 键只有等宽排法，无法使用共享偏好 `touch_fourteen_key_letter_widths` 的「宽窄」布局；设置页和键盘也没有把这项同步偏好展示或应用。宽窄排法的第二排两端必须仍能点到同排最外侧按键，不能产生死区。

## Decision

iOS 复用共享文档字段 `touch_fourteen_key_letter_widths`，缺少或不认识的值按 `equal`。原生设置页在 14/18 键分段增加「字母键宽度」的「等宽」「宽窄」两枚胶囊，保存后更新 App Group 镜像。键盘从镜像读值，在现有行视图上重新应用字母键份额；宽窄模式按双字母键 1 份、单字母键 0.75 份计算，短排两端增加透明占位格，并由 `KeyRoutingSpacer` 将触摸转发给同排最外侧按键。双字母键面使用字距属性分开字母，键面字符串和无障碍标签保持不变。

设置页预览和皮肤预览都读取同一偏好，默认等宽时保留原有排法。

## Alternatives considered

- 只在设置页保存偏好、暂不改变键盘：实现简单但用户选择宽窄后没有行为变化，不能满足 issue。
- 用占位 `UIView` 但不转发触摸：会把第二排两端空白变成死区，违反 `KeyGapRouting` 的输入边界约定。

## Consequences

宽窄偏好现在能从 iOS 设置页写入共享文档，并在键盘重载时生效；空白点按会沿同排转发到边缘键。已有用户缺字段仍保持等宽。由于当前工作区没有暂存的 iOS EngineResources，XcodeGen 无法生成最新工程，真机和模拟器构建尚未完成，需要在具备资源目录的环境补跑 iOS 定向测试和 UI 截图验收。
