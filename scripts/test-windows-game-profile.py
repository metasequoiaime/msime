#!/usr/bin/env python3
"""在合成注册表上执行实际登记名称读写与回滚函数，不修改本机注册表。"""
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARNESS = r'''
#include <cassert>
#include <cstdint>
#include <cwchar>
#include <cstring>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>
#include "msime_edition.h"
using DWORD = uint32_t;
using BYTE = unsigned char;
using REGSAM = unsigned;
constexpr unsigned KEY_WOW64_64KEY = 0x100, KEY_WOW64_32KEY = 0x200;
constexpr unsigned KEY_QUERY_VALUE = 1, KEY_SET_VALUE = 2;
constexpr unsigned ERROR_SUCCESS = 0, ERROR_FILE_NOT_FOUND = 2, ERROR_PATH_NOT_FOUND = 3;
constexpr unsigned REG_SZ = 1;
struct Key { std::map<std::wstring, std::wstring> values; bool denied = false; };
using HKEY = Key*;
const HKEY HKEY_LOCAL_MACHINE = nullptr;
std::map<std::pair<std::wstring, unsigned>, Key> keys;
std::set<unsigned> failWrites;
unsigned writes = 0;
DWORD RegOpenKeyExW(HKEY, const wchar_t *path, int, unsigned access, HKEY *out) {
 auto found = keys.find({path, access & 0x300});
 if (found == keys.end()) return ERROR_FILE_NOT_FOUND;
 if (found->second.denied) return 5;
 *out = &found->second; return 0;
}
DWORD RegQueryValueExW(HKEY key, const wchar_t *name, void*, DWORD *type, BYTE *out, DWORD *bytes) {
 auto found = key->values.find(name);
 if (found == key->values.end()) return ERROR_FILE_NOT_FOUND;
 const auto size = (found->second.size() + 1) * sizeof(wchar_t);
 if (*bytes < size) return 234;
 *bytes = static_cast<DWORD>(size); *type = REG_SZ;
 std::memcpy(out, found->second.c_str(), size); return 0;
}
void RegCloseKey(HKEY) {}
DWORD RegDeleteValueW(HKEY key, const wchar_t *name) {
 if (failWrites.count(++writes)) return 5;
 return key->values.erase(name) ? 0 : ERROR_FILE_NOT_FOUND;
}
DWORD RegSetValueExW(HKEY key, const wchar_t *name, int, unsigned, const BYTE *data, DWORD) {
 if (failWrites.count(++writes)) return 5;
 key->values[name] = reinterpret_cast<const wchar_t*>(data); return 0;
}
DWORD RegCreateKeyExW(HKEY, const wchar_t *path, int, void*, int, unsigned access, void*, HKEY *out, void*) {
 auto &key = keys[{path, access & 0x300}];
 if (key.denied) return 5;
 *out = &key; return 0;
}
@PRODUCTION@
using namespace msime::windows::game_profile;
Key &profile(unsigned view) { return keys[{profile_key(), view}]; }
Key &record() { return keys[{MSIME_EDITION_REGISTRY_KEY, KEY_WOW64_64KEY}]; }
void reset() {
 keys.clear(); failWrites.clear(); writes = 0;
 profile(KEY_WOW64_64KEY).values[L"Description"] = L"old64";
 profile(KEY_WOW64_32KEY).values[L"Description"] = L"old32";
 // 不同产品版本和 msime-windows 的记录必须保持原样。
 keys[{L"Software\\Metasequoia\\Unrelated", KEY_WOW64_64KEY}].values[L"GameCompatProfileName"] = L"other";
}
void old_profiles() {
 assert(profile(KEY_WOW64_64KEY).values.at(L"Description") == L"old64");
 assert(profile(KEY_WOW64_32KEY).values.at(L"Description") == L"old32");
}
int main() {
 reset();
 assert(profile_key().find(MSIME_EDITION_CLSID_STRING) != std::wstring::npos);
 assert(profile_key().find(MSIME_EDITION_PROFILE_GUID_STRING) != std::wstring::npos);
 assert(profile_key().find(L"0x00000804") != std::wstring::npos);
 assert(registration_name() == MSIME_EDITION_TEXT_SERVICE_DESCRIPTION);
 assert(apply_elevated(true) == 0 && enabled());
 assert(profile(KEY_WOW64_64KEY).values.at(L"Description") == CompatName);
 assert(profile(KEY_WOW64_32KEY).values.at(L"Description") == CompatName);
 assert(registration_name() == CompatName); // 升级注册仍读取持久记录。
 assert(apply_elevated(false) == 0 && !enabled());
 assert(profile(KEY_WOW64_64KEY).values.at(L"Description") == MSIME_EDITION_TEXT_SERVICE_DESCRIPTION);
 assert(profile(KEY_WOW64_32KEY).values.at(L"Description") == MSIME_EDITION_TEXT_SERVICE_DESCRIPTION);
 assert(keys.at({L"Software\\Metasequoia\\Unrelated", KEY_WOW64_64KEY}).values.at(L"GameCompatProfileName") == L"other");
 reset(); profile(KEY_WOW64_32KEY).denied = true;
 assert(apply_elevated(true) == 3 && writes == 0); old_profiles();
 reset(); keys.erase({profile_key(), KEY_WOW64_64KEY}); keys.erase({profile_key(), KEY_WOW64_32KEY});
 assert(apply_elevated(true) == 4 && writes == 0);
 reset(); profile(KEY_WOW64_32KEY).values.clear();
 assert(apply_elevated(true) == 5 && writes == 0);
 for (const unsigned failure : {1u, 2u, 3u}) {
  reset(); failWrites = {failure};
  assert(apply_elevated(true) == 5 && !enabled()); old_profiles();
 }
 reset(); assert(apply_elevated(true) == 0); writes = 0; failWrites = {3};
 assert(apply_elevated(false) == 5 && enabled());
 assert(profile(KEY_WOW64_64KEY).values.at(L"Description") == CompatName);
 assert(profile(KEY_WOW64_32KEY).values.at(L"Description") == CompatName);
 reset(); failWrites = {2, 3}; assert(apply_elevated(true) == 6);
 reset(); record().values[RecordValue] = L"untrusted-custom-name";
 assert(!enabled() && registration_name() == MSIME_EDITION_TEXT_SERVICE_DESCRIPTION);
 reset(); keys.erase({profile_key(), KEY_WOW64_32KEY});
 assert(apply_elevated(true) == 0 && enabled());
}
'''


def main() -> int:
    compiler = shutil.which("c++")
    if not compiler:
        print("skipped: no host C++ compiler")
        return 0
    header = (ROOT / "platforms/windows/common/GameCompatProfile.h").read_text()
    production = header[header.index("namespace msime::windows::game_profile {"):header.index("inline int run_if_requested(")] + "}\n"
    with tempfile.TemporaryDirectory(prefix="msime-game-profile-") as directory:
        source = pathlib.Path(directory) / "test.cpp"
        source.write_text(HARNESS.replace("@PRODUCTION@", production))
        for edition in ("FULL", "PINYIN"):
            binary = pathlib.Path(directory) / edition
            subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-DMSIME_EDITION_" + edition,
                            "-I" + str(ROOT / "shared/contracts"), str(source), "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)
    print("Windows game profile: edition isolation, both registry views, upgrade persistence and failed-write rollback passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
