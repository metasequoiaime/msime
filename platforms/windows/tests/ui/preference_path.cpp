#include "../../settings/PreferencePath.h"
#include <cstdio>
#include <map>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

using namespace msime::settings;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Preference path test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

// 一棵只有对象和字符串的假 JSON 树，对象按引用共享，和 WinRT 的 JsonObject 一样。
struct Node;
using Object = std::shared_ptr<Node>;
struct Entry {
  Object object;
  std::wstring text;
};
struct Node {
  std::map<std::wstring, Entry> entries;
};

struct FakeTree {
  using Object = ::Object;
  using Value = std::optional<Entry>;
  static std::optional<Object> object(Object const &parent, std::wstring const &name) {
    const auto found = parent->entries.find(name);
    if (found == parent->entries.end() || !found->second.object)
      return std::nullopt;
    return found->second.object;
  }
  static Value value(Object const &parent, std::wstring const &name) {
    const auto found = parent->entries.find(name);
    if (found == parent->entries.end())
      return std::nullopt;
    return found->second;
  }
  static Value none() { return std::nullopt; }
  static Object copy(Object const &source) {
    auto result = std::make_shared<Node>();
    for (auto const &[name, entry] : source->entries)
      result->entries[name] = entry.object ? Entry{copy(entry.object), {}} : entry;
    return result;
  }
  static Object empty() { return std::make_shared<Node>(); }
  static void set(Object const &parent, std::wstring const &name, Value const &value) {
    parent->entries[name] = *value;
  }
  static void set_object(Object const &parent, std::wstring const &name, Object const &child) {
    parent->entries[name] = Entry{child, {}};
  }
  static void remove(Object const &parent, std::wstring const &name) {
    parent->entries.erase(name);
  }
};

Entry text(std::wstring value) { return Entry{nullptr, std::move(value)}; }

std::wstring read(Object const &root, std::vector<std::wstring> const &path) {
  const auto value = find_preference<FakeTree>(root, path);
  return value && !value->object ? value->text : L"<none>";
}

int main() {
  try {
    // 点分路径拆成各段；不合法的路径拆成空表。
    require(preference_path(L"global_theme") == std::vector<std::wstring>{L"global_theme"});
    require(preference_path(L"plugins.key_sound.enabled") ==
            std::vector<std::wstring>{L"plugins", L"key_sound", L"enabled"});
    for (auto bad : {L"", L".", L".a", L"a.", L"a..b"})
      require(preference_path(bad).empty());

    // 表项的键原样作为最后一段，里面的点不拆。
    require(preference_entry_path(L"ai_assistant.tokens", L"https://api.example.com") ==
            std::vector<std::wstring>{L"ai_assistant", L"tokens", L"https://api.example.com"});
    require(preference_entry_path(L"a..b", L"x").empty());

    // 默认值：plugins.key_sound 有 enabled 和 pack 两项，custom_theme.candidate_colors 有 surface。
    auto defaults = FakeTree::empty();
    {
      auto plugins = FakeTree::empty();
      auto key_sound = FakeTree::empty();
      key_sound->entries[L"enabled"] = text(L"false");
      key_sound->entries[L"pack"] = text(L"typewriter");
      plugins->entries[L"key_sound"] = Entry{key_sound, {}};
      defaults->entries[L"plugins"] = Entry{plugins, {}};
      auto theme = FakeTree::empty();
      auto colors = FakeTree::empty();
      colors->entries[L"surface"] = text(L"#ffffff");
      theme->entries[L"candidate_colors"] = Entry{colors, {}};
      defaults->entries[L"custom_theme"] = Entry{theme, {}};
    }

    // 读：任意深；中途缺了、中途不是对象、路径不合法都读不到。
    require(read(defaults, preference_path(L"plugins.key_sound.pack")) == L"typewriter");
    require(read(defaults, preference_path(L"plugins.missing.pack")) == L"<none>");
    require(read(defaults, preference_path(L"plugins.key_sound.pack.deeper")) == L"<none>");
    require(read(defaults, {}) == L"<none>");
    require(!find_preference<FakeTree>(std::nullopt, preference_path(L"plugins")));

    // 写：缺的中间层从默认值同一路径的对象复制而来，写入不改默认值本身。
    auto stored = FakeTree::empty();
    set_preference<FakeTree>(stored, defaults, preference_path(L"plugins.key_sound.enabled"),
                             text(L"true"));
    require(read(stored, preference_path(L"plugins.key_sound.enabled")) == L"true");
    require(read(stored, preference_path(L"plugins.key_sound.pack")) == L"typewriter");
    require(read(defaults, preference_path(L"plugins.key_sound.enabled")) == L"false");

    // 已有的中间层原样沿用，只改这一项。
    set_preference<FakeTree>(stored, defaults, preference_path(L"plugins.key_sound.pack"),
                             text(L"piano"));
    require(read(stored, preference_path(L"plugins.key_sound.enabled")) == L"true");
    require(read(stored, preference_path(L"plugins.key_sound.pack")) == L"piano");

    // 存储里有上一层、缺下一层时，下一层仍从默认值补。
    auto partial = FakeTree::empty();
    partial->entries[L"custom_theme"] = Entry{FakeTree::empty(), {}};
    set_preference<FakeTree>(partial, defaults, preference_path(L"custom_theme.candidate_colors.text"),
                             text(L"#000000"));
    require(read(partial, preference_path(L"custom_theme.candidate_colors.surface")) == L"#ffffff");
    require(read(partial, preference_path(L"custom_theme.candidate_colors.text")) == L"#000000");

    // 默认值里也没有的中间层是空对象；挡路的非对象值换成对象。
    set_preference<FakeTree>(stored, defaults, preference_path(L"brand.new.leaf"), text(L"1"));
    require(read(stored, preference_path(L"brand.new.leaf")) == L"1");
    require(stored->entries[L"brand"].object->entries.size() == 1);
    stored->entries[L"scalar"] = text(L"x");
    set_preference<FakeTree>(stored, std::nullopt, preference_path(L"scalar.leaf"), text(L"2"));
    require(read(stored, preference_path(L"scalar.leaf")) == L"2");

    // 不合法的路径什么也不写。
    const auto before = stored->entries.size();
    set_preference<FakeTree>(stored, defaults, preference_path(L"a..b"), text(L"3"));
    require(stored->entries.size() == before);

    // 表项：键里的点不拆，按整个键读写。
    const auto token = preference_entry_path(L"ai_assistant.tokens", L"https://api.example.com");
    set_preference<FakeTree>(stored, defaults, token, text(L"secret"));
    require(read(stored, token) == L"secret");
    require(stored->entries[L"ai_assistant"].object->entries[L"tokens"].object->entries.count(
                L"https://api.example.com") == 1);
    require(read(stored, preference_path(L"ai_assistant.tokens.https://api.example.com")) ==
            L"<none>");

    // 删：任意深删掉一项，旁边的不动；不存在的路径不补中间层。
    remove_preference<FakeTree>(stored, token);
    require(read(stored, token) == L"<none>");
    require(stored->entries[L"ai_assistant"].object->entries.count(L"tokens") == 1);
    remove_preference<FakeTree>(stored, preference_path(L"plugins.key_sound.pack"));
    require(read(stored, preference_path(L"plugins.key_sound.pack")) == L"<none>");
    require(read(stored, preference_path(L"plugins.key_sound.enabled")) == L"true");
    remove_preference<FakeTree>(stored, preference_path(L"nowhere.at.all"));
    require(stored->entries.count(L"nowhere") == 0);
    remove_preference<FakeTree>(stored, preference_path(L"scalar"));
    require(stored->entries.count(L"scalar") == 0);
    return 0;
  } catch (std::exception const &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
}
