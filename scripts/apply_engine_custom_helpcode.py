#!/usr/bin/env python3
"""Allow user helper-code tables below ``helpcodes/custom`` in the pinned Engine.

The Rust client discovers and labels these files. This overlay keeps the Engine's existing
candidate filtering path authoritative for the actual table contents and accepts the same schema
identifier (`custom/<file stem>`).
"""

from pathlib import Path


def replace_once(path: Path, before: str, after: str, marker: str) -> None:
    text = path.read_text(encoding="utf-8")
    if marker in text:
        return
    if text.count(before) != 1:
        raise RuntimeError(f"Engine overlay expected one match in {path}")
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def apply(root: Path) -> None:
    header = root / "common/helpcode_utils.h"
    replace_once(
        header,
        "bool select_helpcode_schema(const std::string &schema);\n",
        "bool select_helpcode_schema(const std::string &schema);\n"
        "inline constexpr char kCustomHelpcodeSchemaPrefix[] = \"custom/\";\n",
        "kCustomHelpcodeSchemaPrefix",
    )

    source = root / "common/helpcode_utils.cpp"
    replace_once(
        source,
        "bool is_han_code_point(std::uint32_t code_point)\n{\n"
        "    return code_point == 0x3007 || (code_point >= 0x3400 && code_point <= 0x4DBF) ||\n"
        "           (code_point >= 0x4E00 && code_point <= 0x9FFF) || (code_point >= 0xF900 && code_point <= 0xFAFF) ||\n"
        "           (code_point >= 0x20000 && code_point <= 0x2FA1F) || (code_point >= 0x30000 && code_point <= 0x323AF);\n"
        "}\n",
        "bool is_han_code_point(std::uint32_t code_point)\n{\n"
        "    return code_point == 0x3007 || (code_point >= 0x3400 && code_point <= 0x4DBF) ||\n"
        "           (code_point >= 0x4E00 && code_point <= 0x9FFF) || (code_point >= 0xF900 && code_point <= 0xFAFF) ||\n"
        "           (code_point >= 0x20000 && code_point <= 0x2FA1F) || (code_point >= 0x30000 && code_point <= 0x323AF);\n"
        "}\n\n"
        "std::string custom_schema_stem(const std::string &schema)\n{\n"
        "    const std::string prefix = HelpcodeUtils::kCustomHelpcodeSchemaPrefix;\n"
        "    if (schema.size() <= prefix.size() || schema.compare(0, prefix.size(), prefix) != 0)\n"
        "        return {};\n"
        "    const std::string stem = schema.substr(prefix.size());\n"
        "    if (stem.front() == '.' || stem.find_first_of(\"/\\\\:*?\\\"<>|\") != std::string::npos ||\n"
        "        std::any_of(stem.begin(), stem.end(), [](unsigned char ch) { return ch < 0x20; }))\n"
        "        return {};\n"
        "    return stem;\n"
        "}\n\n"
        "std::filesystem::path custom_schema_file(const std::filesystem::path &resources, const std::string &stem)\n{\n"
        "    return resources / \"helpcodes\" / \"custom\" / (stem + \".txt\");\n"
        "}\n",
        "std::filesystem::path custom_schema_file(const std::filesystem::path",
    )

    replace_once(
        source,
        "    const auto found = std::find_if(metasequoia::assets::helpcodes.begin(), metasequoia::assets::helpcodes.end(),\n"
        "                                    [&](const auto &entry) { return entry.schema == schema; });\n"
        "    if (found == metasequoia::assets::helpcodes.end())\n"
        "        throw std::invalid_argument(\"Unknown helpcode schema\");\n"
        "    auto result = std::make_shared<Keymap>();\n"
        "    std::ifstream input(resources / found->path);\n"
        "    std::string line;\n"
        "    while (std::getline(input, line))\n"
        "    {\n"
        "        const auto pos = line.find('=');\n",
        "    const auto found = std::find_if(metasequoia::assets::helpcodes.begin(), metasequoia::assets::helpcodes.end(),\n"
        "                                    [&](const auto &entry) { return entry.schema == schema; });\n"
        "    std::filesystem::path file;\n"
        "    if (found != metasequoia::assets::helpcodes.end())\n"
        "        file = resources / found->path;\n"
        "    else if (const auto stem = custom_schema_stem(schema); !stem.empty() &&\n"
        "             std::filesystem::is_regular_file(custom_schema_file(resources, stem)))\n"
        "        file = custom_schema_file(resources, stem);\n"
        "    else\n"
        "        throw std::invalid_argument(\"Unknown helpcode schema\");\n"
        "    auto result = std::make_shared<Keymap>();\n"
        "    std::ifstream input(file);\n"
        "    std::string line;\n"
        "    bool first_line = true;\n"
        "    while (std::getline(input, line))\n"
        "    {\n"
        "        if (first_line && line.rfind(\"\\xEF\\xBB\\xBF\", 0) == 0)\n"
        "            line.erase(0, 3);\n"
        "        first_line = false;\n"
        "        if (!line.empty() && line.back() == '\\r')\n"
        "            line.pop_back();\n"
        "        if (!line.empty() && line.front() == '#')\n"
        "            continue;\n"
        "        const auto pos = line.find('=');\n",
        "bool first_line = true;",
    )

    replace_once(
        source,
        "bool is_supported_helpcode_schema(const std::string &schema)\n{\n"
        "    return std::any_of(metasequoia::assets::helpcodes.begin(), metasequoia::assets::helpcodes.end(),\n"
        "                       [&](const auto &entry) { return entry.schema == schema; });\n"
        "}\n",
        "bool is_supported_helpcode_schema(const std::string &schema)\n{\n"
        "    if (std::any_of(metasequoia::assets::helpcodes.begin(), metasequoia::assets::helpcodes.end(),\n"
        "                    [&](const auto &entry) { return entry.schema == schema; }))\n"
        "        return true;\n"
        "    const auto stem = custom_schema_stem(schema);\n"
        "    return !stem.empty();\n"
        "}\n",
        "    return !stem.empty();",
    )


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
