#pragma once

#include "PanelContext.h"
#include <functional>
#include <stdexcept>
#include <set>

namespace msime::windows::win7::emoji {
struct Item { std::string text, annotation, tab, group; };
struct Group { std::string id, tab, title; };
struct Catalog { std::vector<Item> items; std::vector<Group> groups; std::vector<std::wstring> errors; };
using Transport = std::function<std::string(const std::string&, const std::string&)>;
inline bool valid_text(const Json& value, size_t limit = 16384) {
  if (!value.is_string()) return false;
  const auto& text = value.get_ref<const std::string&>();
  return text.size() <= limit && text.find('\0') == std::string::npos;
}
inline Json request(const Json& query, const std::string& resources, const Transport& transport) {
  const auto bytes = query.dump();
  const auto response = transport ? Json::parse(transport(bytes, resources), nullptr, false) :
      msime::settings::win7::theme_data::response(msime_client_emoji_catalog_request(
        reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size(), reinterpret_cast<const uint8_t*>(resources.data()), resources.size()));
  const auto value = msime::settings::win7::theme_data::value(response);
  if (!value || !value->is_object()) throw std::runtime_error("emoji catalog unavailable");
  return *value;
}
inline void read_pages(Catalog& catalog, Json query, const std::string& resources, const Transport& transport,
                       const std::string& tab, const std::string& fixed_group = {}) {
  size_t offset = 0;
  for (size_t page = 0; page < 4096; ++page) {
    query["cursor"] = true; query["offset"] = offset; query["limit"] = 255;
    const auto value = request(query, resources, transport);
    if (!value.contains("items") || !value.at("items").is_array() || value.at("items").size() > 255 ||
        !value.contains("next_offset") || !value.at("next_offset").is_number_unsigned() ||
        !value.contains("complete") || !value.at("complete").is_boolean()) throw std::runtime_error("invalid emoji page");
    for (const auto& row : value.at("items")) {
      if (!row.is_object()) throw std::runtime_error("invalid emoji row");
      for (const auto* field : {"text", "annotation", "group"})
        if (!row.contains(field) || !valid_text(row.at(field))) throw std::runtime_error("invalid emoji row");
      if (row.at("text").get_ref<const std::string&>().empty() || catalog.items.size() >= 200000) throw std::runtime_error("invalid emoji row");
      const auto group = fixed_group.empty() ? "builtin:" + tab + ":" + row.at("group").get<std::string>() : fixed_group;
      catalog.items.push_back({row.at("text"), row.at("annotation"), tab, group});
    }
    const auto next = value.at("next_offset").get<size_t>();
    if (next < offset || next - offset > 255) throw std::runtime_error("invalid emoji cursor");
    if (value.at("complete").get<bool>()) return;
    if (next == offset) throw std::runtime_error("stalled emoji cursor");
    offset = next;
  }
  throw std::runtime_error("emoji catalog too large");
}
inline Catalog load(const PanelContext& context, const Transport& transport) {
  Catalog result;
  const auto resources = context.options.is_object() && context.options.contains("resources") && valid_text(context.options.at("resources")) ? context.options.at("resources").get<std::string>() : context.directory.u8string();
  for (const auto* tab : {"emoji", "kaomoji", "symbols"}) {
    Catalog part;
    try {
      if (std::string_view(tab) == "symbols") {
        const auto value = request({{"list_symbol_groups", true}}, resources, transport);
        if (!value.contains("symbol_groups") || !value.at("symbol_groups").is_array() || value.at("symbol_groups").size() > 4096) throw std::runtime_error("invalid symbol groups");
        for (const auto& group : value.at("symbol_groups")) {
          if (!group.is_object() || !group.contains("parent") || !valid_text(group.at("parent")) || !group.contains("title") || !valid_text(group.at("title"))) throw std::runtime_error("invalid symbol group");
          const auto parent = group.at("parent").get<std::string>(), title = group.at("title").get<std::string>();
          const auto id = "builtin:symbols:" + Json::array({parent,title}).dump();
          part.groups.push_back({id,tab,parent.empty() ? title : parent + " / " + title});
          read_pages(part, {{"category","symbols"},{"parent",parent},{"group",title}}, resources, transport, tab, id);
        }
      } else {
        const auto category = std::string_view(tab) == "kaomoji" ? "kaomoji" : "";
        read_pages(part, {{"category", category}}, resources, transport, tab);
        std::set<std::string> seen;
        for (const auto& item : part.items) if (seen.insert(item.group).second) {
          const auto title = item.group.substr(std::string("builtin:").size() + std::string(tab).size() + 1);
          part.groups.push_back({item.group, tab, std::string_view(tab) == "kaomoji" ? "全部颜文字" : title});
        }
      }
      result.items.insert(result.items.end(), part.items.begin(), part.items.end());
      result.groups.insert(result.groups.end(), part.groups.begin(), part.groups.end());
    } catch (const std::exception&) {
      result.errors.push_back(std::string_view(tab) == "emoji" ? L"内置表情目录无法读取。" : std::string_view(tab) == "kaomoji" ? L"内置颜文字目录无法读取。" : L"内置符号目录无法读取。");
    }
  }
  try {
    const auto value = request({{"list_plugin_symbol_groups",true},{"plugins",(context.directory / L"plugins").u8string()}}, resources, transport);
    if (!value.contains("plugin_symbol_groups") || !value.at("plugin_symbol_groups").is_array() || value.at("plugin_symbol_groups").size() > 4096) throw std::runtime_error("invalid plugin symbol groups");
    Catalog part;
    size_t index = 0;
    for (const auto& group : value.at("plugin_symbol_groups")) {
      if (!group.is_object()) throw std::runtime_error("invalid plugin symbol group");
      for (const auto* key : {"pack", "pack_name", "tab", "title", "keywords"}) if (!group.contains(key) || !valid_text(group.at(key))) throw std::runtime_error("invalid plugin symbol group");
      const auto tab = group.at("tab").get<std::string>();
      if ((tab != "symbols" && tab != "kaomoji") || !group.contains("items") || !group.at("items").is_array()) throw std::runtime_error("invalid plugin symbols");
      const auto id = "plugin:" + std::to_string(index++);
      part.groups.push_back({id,tab,group.at("pack_name").get<std::string>() + " / " + group.at("title").get<std::string>()});
      for (const auto& text : group.at("items")) {
        if (!valid_text(text) || text.get_ref<const std::string&>().empty() || part.items.size() >= 200000) throw std::runtime_error("invalid plugin symbol");
        part.items.push_back({text,group.at("keywords"),tab,id});
      }
    }
    result.items.insert(result.items.end(), part.items.begin(), part.items.end()); result.groups.insert(result.groups.end(), part.groups.begin(), part.groups.end());
  } catch (const std::exception&) { result.errors.push_back(L"插件符号组无法读取。"); }
  return result;
}
}
