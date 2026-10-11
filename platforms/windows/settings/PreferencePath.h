#pragma once
#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 设置文档里按路径读写一个偏好：`plugins.key_sound.enabled` 这样的点分路径可以任意深，`ai_assistant.tokens` 这类以来源为键的表，键本身可能带点（`https://api.example.com`），所以表项的键单独给出、原样当作最后一段。
// 走路径的逻辑与 JSON 的实现无关：调用方提供一个 Tree（窗口里是 WinRT 的 JsonObject，测试里是一棵假的树），这样不依赖 Windows 和 WinRT 头文件，方便单独测试。
// Tree 需要提供：
//   using Object = ...;  对象，按引用语义共享（改子对象即改原文档）
//   using Value = ...;   值，缺省构造或 Tree::none() 表示没有
//   static std::optional<Object> object(Object const &, std::wstring const &name);  子项存在且是对象时返回它
//   static Value value(Object const &, std::wstring const &name);                   子项，没有时返回 none()
//   static Value none();
//   static Object copy(Object const &);    深拷贝
//   static Object empty();                 空对象
//   static void set(Object const &, std::wstring const &name, Value const &);
//   static void set_object(Object const &, std::wstring const &name, Object const &);
//   static void remove(Object const &, std::wstring const &name);  没有这一项时什么也不做
namespace msime::settings {

// 把点分路径拆成各段。空路径、开头或结尾是点、连续两个点都不是合法路径，返回空表，调用方据此什么也不做。
inline std::vector<std::wstring> preference_path(std::wstring_view key) {
  std::vector<std::wstring> segments;
  if (key.empty())
    return segments;
  std::size_t start = 0;
  while (true) {
    const auto dot = key.find(L'.', start);
    const auto segment = key.substr(start, dot == std::wstring_view::npos ? std::wstring_view::npos : dot - start);
    if (segment.empty())
      return {};
    segments.emplace_back(segment);
    if (dot == std::wstring_view::npos)
      return segments;
    start = dot + 1;
  }
}

// 表里一项的路径：表的路径照常按点拆开，表项的键原样作为最后一段，里面的点不再拆。表的路径不合法时返回空表。
inline std::vector<std::wstring> preference_entry_path(std::wstring_view map_key,
                                                       std::wstring_view entry) {
  auto segments = preference_path(map_key);
  if (!segments.empty())
    segments.emplace_back(entry);
  return segments;
}

// 路径上的值；中途缺了某一段或某一段不是对象时返回 none()。
template <class Tree>
typename Tree::Value find_preference(std::optional<typename Tree::Object> root,
                                     std::vector<std::wstring> const &path) {
  if (!root || path.empty())
    return Tree::none();
  auto object = *root;
  for (std::size_t index = 0; index + 1 < path.size(); ++index) {
    auto child = Tree::object(object, path[index]);
    if (!child)
      return Tree::none();
    object = *child;
  }
  return Tree::value(object, path.back());
}

// 路径最后一段所在的对象。create 为 false 时中途缺了就返回空；为 true 时补上缺的（或不是对象的）中间层。
// 存储只整体接受一个嵌套对象，所以补上的那一层从随包默认值里同一路径的对象复制而来，默认值里也没有才是空对象。
template <class Tree>
std::optional<typename Tree::Object>
preference_parent(typename Tree::Object const &root,
                  std::optional<typename Tree::Object> defaults,
                  std::vector<std::wstring> const &path, bool create) {
  if (path.empty())
    return std::nullopt;
  auto object = root;
  for (std::size_t index = 0; index + 1 < path.size(); ++index) {
    auto const &name = path[index];
    if (defaults)
      defaults = Tree::object(*defaults, name);
    if (auto child = Tree::object(object, name)) {
      object = *child;
      continue;
    }
    if (!create)
      return std::nullopt;
    auto nested = defaults ? Tree::copy(*defaults) : Tree::empty();
    Tree::set_object(object, name, nested);
    object = nested;
  }
  return object;
}

// 把值写到路径上，按上面的规则补齐中间层。路径不合法时什么也不做。
template <class Tree>
void set_preference(typename Tree::Object const &root,
                    std::optional<typename Tree::Object> const &defaults,
                    std::vector<std::wstring> const &path,
                    typename Tree::Value const &value) {
  if (auto parent = preference_parent<Tree>(root, defaults, path, true))
    Tree::set(*parent, path.back(), value);
}

// 删掉路径上的那一项；中途缺了就说明本来就没有，不补中间层。
template <class Tree>
void remove_preference(typename Tree::Object const &root, std::vector<std::wstring> const &path) {
  if (auto parent = preference_parent<Tree>(root, std::nullopt, path, false))
    Tree::remove(*parent, path.back());
}

} // namespace msime::settings
