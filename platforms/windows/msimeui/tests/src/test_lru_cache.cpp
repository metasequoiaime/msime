#include "tests/includes/test_framework.h"

#include "msimeui/LruCache.h"

#include <string>

#include <unknwn.h>
#include <wrl/client.h>

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

namespace
{
class CountedUnknown final : public IUnknown
{
  public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void **object) override
    {
        if (!object)
        {
            return E_POINTER;
        }
        *object = iid == __uuidof(IUnknown) ? this : nullptr;
        if (!*object)
        {
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override
    {
        return ++references_;
    }
    ULONG STDMETHODCALLTYPE Release() override
    {
        return --references_;
    }

  private:
    ULONG references_ = 1;
};
} // namespace

// The text format cache stores WRL ComPtrs, whose overloaded operator& hands
// back a reference that empties the ComPtr once converted to a pointer. Find
// must return the stored value intact, every time it is asked.
TEST_CASE(lru_cache_find_keeps_a_com_pointer)
{
    CountedUnknown object;
    Microsoft::WRL::ComPtr<IUnknown> value(&object);
    LruCache<std::wstring, Microsoft::WRL::ComPtr<IUnknown>> cache(2);
    cache.Insert(L"format", value);

    REQUIRE(cache.Find(L"format") != nullptr);
    REQUIRE(cache.Find(L"format")->Get() == &object);
    REQUIRE(cache.Find(L"format")->Get() == &object);
}
