#pragma once

#include <cstddef>
#include <memory>
#include <utility>
#include <vector>

namespace msimeui
{
inline constexpr std::size_t kTextFormatCacheCapacity = 128;

// A small UI cache whose entries are kept alive in most-recently-used order.
// The cache owns the values and evicts the oldest entry before it grows past
// the configured capacity.
template <typename Key, typename Value>
class LruCache
{
  public:
    explicit LruCache(std::size_t capacity) : capacity_(capacity)
    {
    }

    Value *Find(const Key &key)
    {
        for (std::size_t index = 0; index < entries_.size(); ++index)
        {
            if (!(entries_[index].key == key))
            {
                continue;
            }

            if (index + 1 != entries_.size())
            {
                Entry entry = std::move(entries_[index]);
                entries_.erase(entries_.begin() + static_cast<std::ptrdiff_t>(index));
                entries_.push_back(std::move(entry));
            }
            // Not `&`: WRL ComPtr overloads it, and the ComPtrRef it returns
            // empties the ComPtr when converted to a ComPtr pointer.
            return std::addressof(entries_.back().value);
        }
        return nullptr;
    }

    void Insert(Key key, Value value)
    {
        if (capacity_ == 0)
        {
            return;
        }

        for (std::size_t index = 0; index < entries_.size(); ++index)
        {
            if (entries_[index].key == key)
            {
                entries_.erase(entries_.begin() + static_cast<std::ptrdiff_t>(index));
                break;
            }
        }

        if (entries_.size() == capacity_)
        {
            entries_.erase(entries_.begin());
        }
        entries_.push_back({std::move(key), std::move(value)});
    }

    std::size_t Size() const
    {
        return entries_.size();
    }

  private:
    struct Entry
    {
        Key key;
        Value value;
    };

    std::size_t capacity_;
    std::vector<Entry> entries_;
};
} // namespace msimeui
