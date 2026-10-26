#include "../../src/core/CaretEditSuffix.h"

#include <cassert>
#include <string>

using Json = nlohmann::json;
namespace host = msime::linux_host;

namespace {

Json candidate(const char *text, Json index) {
  return Json{{"text", text}, {"id", Json{{"session", 1}, {"generation", 1}, {"index", std::move(index)}}}};
}

} // namespace

int main() {
  // 字段缺失（运行时没有生效时不写这两个键）、视图不是对象、类型不对，都按没有生效处理，不抛异常。
  {
    assert(!host::caret_edit_suffix(Json()).active);
    assert(!host::caret_edit_suffix(Json::array()).active);
    const auto absent = host::caret_edit_suffix(Json{{"editing_text", "chihuoguo"}});
    assert(!absent.active && absent.prefix.empty());
    assert(!host::caret_edit_suffix(Json{{"caret_edit_suffix", false}}).active);
    assert(!host::caret_edit_suffix(Json{{"caret_edit_suffix", 1}}).active);
    assert(!host::caret_edit_suffix(Json{{"caret_edit_suffix", "true"}}).active);
    // 没有生效时前缀不算数，宿主不会按一份残留的前缀去画淡。
    const auto stale = host::caret_edit_suffix(Json{{"caret_edit_suffix", false}, {"caret_edit_prefix", "我好现"}});
    assert(!stale.active && stale.prefix.empty());
  }

  // 生效：前缀可以缺（光标在句首、还没有暂存的部分），也可以有。
  {
    const auto bare = host::caret_edit_suffix(Json{{"caret_edit_suffix", true}});
    assert(bare.active && bare.prefix.empty());
    const auto mode = host::caret_edit_suffix(Json{{"caret_edit_suffix", true}, {"caret_edit_prefix", "我好现"}});
    assert(mode.active && mode.prefix == "我好现");
    const auto odd = host::caret_edit_suffix(Json{{"caret_edit_suffix", true}, {"caret_edit_prefix", 3}});
    assert(odd.active && odd.prefix.empty());
  }

  // 提示只在生效时出现，接在已有的辅助行后面，用宿主自己的分隔符；辅助行原来是空的就只有提示。
  {
    const host::CaretEditSuffix off;
    std::string aux = "1 / 3";
    host::append_caret_edit_suffix_hint(aux, off, " · ");
    assert(aux == "1 / 3");
    std::string empty;
    host::append_caret_edit_suffix_hint(empty, off, " · ");
    assert(empty.empty());

    const host::CaretEditSuffix on{true, "我好现"};
    host::append_caret_edit_suffix_hint(aux, on, " · ");
    assert(aux == "1 / 3 · 候选编辑：光标后");
    std::string ibus = "1/3  · 快捷短语";
    host::append_caret_edit_suffix_hint(ibus, on, "  · ");
    assert(ibus == "1/3  · 快捷短语  · 候选编辑：光标后");
    host::append_caret_edit_suffix_hint(empty, on, " · ");
    assert(empty == "候选编辑：光标后");
    // 没有前缀也照样提示：模式生效与否只看 caret_edit_suffix。
    std::string bare;
    host::append_caret_edit_suffix_hint(bare, host::CaretEditSuffix{true, {}}, " · ");
    assert(bare == std::string(host::kCaretEditSuffixHint));
  }

  // 画淡的范围：只有首选（id.index 为 0），按 Unicode 标量计；三个汉字是三个标量、九个字节。
  {
    const host::CaretEditSuffix mode{true, "我好现"};
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现吃火锅", 0)) == 3);
    // 解析出来的 0 是无符号的，宿主自己拼的候选里是有符号的，两种都认。
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现吃火锅", Json(std::int64_t{0}))) == 3);
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现吃火锅", Json(std::uint64_t{0}))) == 3);
    // 别的行（第 0 页的其余行、别的页）不画。
    assert(host::caret_edit_dim_scalars(mode, candidate("吃火锅", 1)) == 0);
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现吃火锅", 9)) == 0);
    // 冒充成 0 号的别的行（IBus 临时译文页的第一条）开头不是这段前缀，不画。
    assert(host::caret_edit_dim_scalars(mode, candidate("I want hot pot", 0)) == 0);
    assert(host::caret_edit_dim_scalars(mode, candidate("我好", 0)) == 0);
    // 首选整行都是固定的那一段时整行画淡。
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现", 0)) == 3);
    // 没有生效、前缀为空、候选形状不对时都不画。
    assert(host::caret_edit_dim_scalars(host::CaretEditSuffix{}, candidate("我好现吃火锅", 0)) == 0);
    assert(host::caret_edit_dim_scalars(host::CaretEditSuffix{true, {}}, candidate("我好现吃火锅", 0)) == 0);
    assert(host::caret_edit_dim_scalars(mode, Json{{"text", "我好现吃火锅"}}) == 0);
    assert(host::caret_edit_dim_scalars(mode, candidate("我好现吃火锅", "0")) == 0);
    assert(host::caret_edit_dim_scalars(mode, Json("我好现吃火锅")) == 0);
    // 四字节的字（扩展 B）也只算一个标量。
    assert(host::caret_edit_dim_scalars(host::CaretEditSuffix{true, "𠮷a"}, candidate("𠮷a野家", 0)) == 2);
  }

  // 标量换字节：Fcitx5 和 Pango 的区间按字节计；超过文字长度时取整段，不越界。
  {
    assert(host::utf8_scalar_prefix_bytes("我好现吃火锅", 3) == 9);
    assert(host::utf8_scalar_prefix_bytes("我好現喫火鍋", 3) == 9);
    assert(host::utf8_scalar_prefix_bytes("𠮷a野家", 2) == 5);
    assert(host::utf8_scalar_prefix_bytes("我好", 3) == 6);
    assert(host::utf8_scalar_prefix_bytes("我好现", 0) == 0);
    assert(host::utf8_scalar_prefix_bytes("", 3) == 0);
  }
  return 0;
}
