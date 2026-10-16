#include <cstdint>
#include <cstdlib>
#include <cwchar>
#include <iostream>
#include <string>
namespace msime::windows::game_profile {
inline std::wstring selectedName = L"Test IME";
inline std::wstring registration_name() { return selectedName; }
}

using HRESULT = std::int32_t;
using BOOL = bool;
using DWORD = std::uint32_t;
using ULONG = std::uint32_t;
using UINT = unsigned;
using WCHAR = wchar_t;
constexpr HRESULT S_OK = 0, S_FALSE = 1, E_FAIL = -1;
constexpr bool FALSE = false, TRUE = true;
constexpr unsigned MAX_PATH = 260, STRSAFE_MAX_CCH = 65536;
constexpr int CLSID_TF_InputProcessorProfiles = 1, CLSCTX_INPROC_SERVER = 2;
constexpr int IID_ITfInputProcessorProfileMgr = 3, TEXTSERVICE_LANGID = 4;
constexpr int TEXTSERVICE_ICON_INDEX = 5;
namespace Global {
constexpr int dllInstanceHandle = 1, MetasequoiaIMECLSID = 2, MetasequoiaIMEGuidProfile = 3;
}
constexpr bool FAILED(HRESULT result) { return result < 0; }

struct Scenario {
    HRESULT create = S_OK, description = S_OK, registration = S_OK;
    DWORD path_length = 12;
    int releases = 0, registrations = 0;
    ULONG registered_path_length = 0;
    std::wstring name;
} scenario;

struct ITfInputProcessorProfileMgr {
    HRESULT RegisterProfile(int, int, int, const WCHAR *name, ULONG,
                            const WCHAR *path, ULONG path_length, UINT,
                            void *, int, bool, int) {
        ++scenario.registrations;
        scenario.name = name;
        scenario.registered_path_length = path_length;
        if (path[path_length] != L'\0') std::abort();
        return scenario.registration;
    }
    void Release() { ++scenario.releases; }
} manager;

HRESULT CoCreateInstance(int, void *, int, int, void **result) {
    *result = FAILED(scenario.create) ? nullptr : &manager;
    return scenario.create;
}
DWORD GetModuleFileName(int, WCHAR *path, DWORD capacity) {
    // Truncated paths deliberately have no terminator.
    for (DWORD i = 0; i < capacity; ++i) path[i] = L'x';
    if (scenario.path_length < capacity) path[scenario.path_length] = L'\0';
    return scenario.path_length;
}
HRESULT StringCchLength(const WCHAR *text, unsigned, std::size_t *length) {
    *length = std::wcslen(text);
    return scenario.description;
}

#include "registration_profiles.inc"

void require(bool condition, const char *message) {
    if (!condition) { std::cerr << message << '\n'; std::exit(1); }
}

int main() {
    for (DWORD length : {DWORD{0}, DWORD{MAX_PATH}, DWORD{MAX_PATH + 1}}) {
        scenario = {};
        scenario.path_length = length;
        require(!RegisterProfiles(), "invalid module path reported registration success");
        require(scenario.registrations == 0, "invalid path reached RegisterProfile");
        require(scenario.releases == 1, "profile manager leaked on invalid path");
    }
    for (DWORD length : {DWORD{12}, DWORD{MAX_PATH - 1}}) {
        scenario = {};
        scenario.path_length = length;
        require(RegisterProfiles(), "valid path rejected");
        require(scenario.registrations == 1 && scenario.releases == 1, "registration lifecycle wrong");
        require(scenario.registered_path_length == length, "path length changed");
    }
    scenario = {};
    msime::windows::game_profile::selectedName = L"拼音输入法";
    require(RegisterProfiles() && scenario.name == L"拼音输入法", "upgrade lost compatibility name");
    msime::windows::game_profile::selectedName = L"Test IME";
    scenario = {};
    require(RegisterProfiles() && scenario.name == L"Test IME", "disabled name was not restored");
    scenario = {};
    scenario.create = E_FAIL;
    require(!RegisterProfiles() && scenario.releases == 0 && scenario.registrations == 0,
            "COM activation failure was not preserved");
    scenario = {};
    scenario.description = E_FAIL;
    require(!RegisterProfiles() && scenario.releases == 1 && scenario.registrations == 0,
            "description failure was not preserved");
    scenario = {};
    scenario.registration = E_FAIL;
    require(!RegisterProfiles() && scenario.releases == 1 && scenario.registrations == 1,
            "profile registration failure was not preserved");
}
