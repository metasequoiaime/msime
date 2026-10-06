#pragma once

#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

namespace msime::input {

// The senses inside one candidate's gloss.
//
// The packaged dictionary joins several senses of a word with a semicolon - ASCII in the English entries, full-width (U+FF1B) in the Chinese ones. Ctrl+Enter offers them as choices, or commits the first, rather than committing the joined display text. The rule belongs to the dictionary, not to any host: cut on either separator, trim the surrounding whitespace, and drop what is left empty. Newlines are not separators - hosts use them as a column break inside a sense.
//
// macOS, Linux (IBus and Fcitx5) and Windows all answer through this one function; each keeps its own name for it as a forwarder so its callers read the way they always have.
inline std::vector<std::string> gloss_senses(std::string_view gloss)
{
    static constexpr std::string_view fullwidth = "\xEF\xBC\x9B";
    std::size_t separator_count = 0;
    for (std::size_t index = 0; index < gloss.size();) {
        if (gloss[index] == ';')
        {
            ++separator_count;
            ++index;
        }
        else if (gloss.compare(index, fullwidth.size(), fullwidth) == 0)
        {
            ++separator_count;
            index += fullwidth.size();
        }
        else
            ++index;
    }
    std::vector<std::string> senses;
    senses.reserve(separator_count + 1);
    const auto append = [&senses](std::string_view value) {
        const auto first = value.find_first_not_of(" \t\r\n");
        if (first == std::string_view::npos) return;
        const auto last = value.find_last_not_of(" \t\r\n");
        senses.emplace_back(value.substr(first, last - first + 1));
    };
    std::size_t start = 0;
    while (start <= gloss.size())
    {
        const auto ascii = gloss.find(';', start);
        const auto wide = gloss.find(fullwidth, start);
        const auto cut = ascii < wide ? ascii : wide;
        if (cut == std::string_view::npos)
        {
            append(gloss.substr(start));
            break;
        }
        append(gloss.substr(start, cut - start));
        start = cut + (cut == wide ? fullwidth.size() : 1);
    }
    return senses;
}

} // namespace msime::input
