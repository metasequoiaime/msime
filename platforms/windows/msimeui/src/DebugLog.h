#pragma once

#include <cstddef>
#include <string>

namespace msimeui
{
// snprintf reports the length it would have written, which may exceed the
// fixed stack buffer. Leave room for its terminator before handing the count
// to a sink such as WriteFile.
constexpr std::size_t DebugLogWriteLength(int formattedLength, std::size_t bufferCapacity)
{
    if (formattedLength <= 0 || bufferCapacity == 0)
    {
        return 0;
    }
    const std::size_t length = static_cast<std::size_t>(formattedLength);
    return length < bufferCapacity ? length : bufferCapacity - 1;
}

void DebugLog(const std::string &message);
}
