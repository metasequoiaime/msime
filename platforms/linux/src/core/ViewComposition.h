#pragma once

#include <nlohmann/json.hpp>
#include <string>

namespace msime::linux_host {

// Engine 视图里是否还有正在组字的内容：编辑串非空，或者候选列表非空。
//
// IBus 宿主的 `State::view` 在会话打开之前是 JSON null，`State::close()` 也会把它置回 null（英文模式、密码框等被拦截的输入框、偏好保存触发的会话重建、`guarded()` 兜底时都会走到）。nlohmann 的 `value(key, default)` 只在「对象缺这个键」时回退默认值，对象本身是 null 时直接抛 `type_error.306`。这个判断要在没有 `guarded()` 兜底的 glib 定时器里用，那里的异常会一路冒到 `std::terminate`，所以它必须对任何形状的视图都给出答案而不是抛异常：不是对象、键缺失或类型不对，都按「没有组字」处理。
inline bool view_is_composing(const nlohmann::json &view) noexcept {
  if (!view.is_object())
    return false;
  const auto editing = view.find("editing_text");
  if (editing != view.end() && editing->is_string() &&
      !editing->get_ref<const std::string &>().empty())
    return true;
  const auto candidates = view.find("candidates");
  return candidates != view.end() && candidates->is_array() && !candidates->empty();
}

} // namespace msime::linux_host
