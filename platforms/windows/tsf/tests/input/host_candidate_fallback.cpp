#include "../../HostCandidateFallback.h"
#include <cstdlib>

int main()
{
    msime::tsf::EngineView invalid;
    invalid.scheme = msime::windows::scheme::Quanpin;
    invalid.editing_text = "win";
    invalid.local_mode = "none";
    if (msime::tsf::HostCandidateFallback(invalid) != "win")
        return EXIT_FAILURE;

    invalid.candidates.push_back({"id", "你", true, 0, 0});
    if (!msime::tsf::HostCandidateFallback(invalid).empty())
        return EXIT_FAILURE;

    msime::tsf::EngineView korean;
    korean.scheme = msime::windows::scheme::Korean;
    korean.editing_text = "r";
    korean.local_mode = "none";
    if (!msime::tsf::HostCandidateFallback(korean).empty())
        return EXIT_FAILURE;

    msime::tsf::EngineView local;
    local.scheme = msime::windows::scheme::Quanpin;
    local.editing_text = "abc";
    local.local_mode = "temporary_english";
    if (!msime::tsf::HostCandidateFallback(local).empty())
        return EXIT_FAILURE;

    return EXIT_SUCCESS;
}
