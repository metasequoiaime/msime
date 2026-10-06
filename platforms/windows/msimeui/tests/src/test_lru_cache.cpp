#include "tests/includes/test_framework.h"

#include "msimeui/LruCache.h"

#include <string>

using msimeui::LruCache;
using msimeui::kTextFormatCacheCapacity;

TEST_CASE(lru_cache_evicts_the_oldest_entry_at_capacity)
{
    LruCache<std::wstring, int> cache(3);
    cache.Insert(L"first", 1);
    cache.Insert(L"second", 2);
    cache.Insert(L"third", 3);

    REQUIRE(cache.Size() == 3);
    REQUIRE(cache.Find(L"first") != nullptr);
    cache.Insert(L"fourth", 4);

    REQUIRE(cache.Size() == 3);
    REQUIRE(cache.Find(L"second") == nullptr);
    REQUIRE(cache.Find(L"first") != nullptr);
    REQUIRE(cache.Find(L"third") != nullptr);
    REQUIRE(cache.Find(L"fourth") != nullptr);
}

TEST_CASE(lru_cache_refreshes_an_existing_entry_without_growing)
{
    LruCache<std::wstring, int> cache(2);
    cache.Insert(L"first", 1);
    cache.Insert(L"second", 2);
    cache.Insert(L"first", 10);
    cache.Insert(L"third", 3);

    REQUIRE(cache.Size() == 2);
    REQUIRE(cache.Find(L"first") != nullptr);
    REQUIRE(*cache.Find(L"first") == 10);
    REQUIRE(cache.Find(L"second") == nullptr);
    REQUIRE(cache.Find(L"third") != nullptr);
}

TEST_CASE(lru_cache_keeps_unique_text_formats_bounded)
{
    LruCache<std::wstring, int> cache(kTextFormatCacheCapacity);
    for (std::size_t index = 0; index < kTextFormatCacheCapacity + 1; ++index)
    {
        cache.Insert(L"font-" + std::to_wstring(index), static_cast<int>(index));
    }

    REQUIRE(cache.Size() == kTextFormatCacheCapacity);
    REQUIRE(cache.Find(L"font-0") == nullptr);
    REQUIRE(cache.Find(L"font-" + std::to_wstring(kTextFormatCacheCapacity)) != nullptr);
}
