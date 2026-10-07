#include "tests/includes/test_framework.h"

#include "src/tsf/TextContainer.h"

#include <limits>
#include <string>

TEST_CASE(text_container_rejects_invalid_positions_without_mutating_text)
{
    CTextContainer container;
    REQUIRE(container.InsertText(0, L"abc", 3));

    REQUIRE(!container.InsertText(-1, L"x", 1));
    REQUIRE(!container.InsertText(4, L"x", 1));
    REQUIRE(!container.RemoveText(-1, 1));
    REQUIRE(!container.RemoveText(4, 1));

    WCHAR output[4] = {};
    REQUIRE(!container.GetText(-1, output, 1));
    REQUIRE(!container.GetText(4, output, 1));
    REQUIRE(container.GetText(0, output, 3));
    REQUIRE(std::wstring(output, 3) == L"abc");
}

TEST_CASE(text_container_rejects_lengths_that_would_exceed_the_text_limit)
{
    CTextContainer container;
    const std::wstring text(CTextContainer::kMaxTextUnits, L'a');

    REQUIRE(container.InsertText(0, text.data(), static_cast<UINT>(text.size())));
    REQUIRE(container.GetTextLength() == CTextContainer::kMaxTextUnits);
    REQUIRE(!container.InsertText(0, L"x", 1));
    REQUIRE(!container.InsertText(0, L"x", std::numeric_limits<UINT>::max()));
    REQUIRE(container.GetTextLength() == CTextContainer::kMaxTextUnits);
}

TEST_CASE(text_container_handles_empty_operations_without_touching_memory)
{
    CTextContainer container;
    REQUIRE(container.InsertText(0, nullptr, 0));
    REQUIRE(container.RemoveText(0, 0));
    REQUIRE(container.GetTextLength() == 0);
}
