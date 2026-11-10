#include "../../src/input/CharacterSetOutput.h"

#include <cassert>
#include <string>

int main() {
    using namespace msime::mac::character_set;
    assert(follows_cantonese(msime::mac::scheme::Cantonese));
    assert(!follows_cantonese(msime::mac::scheme::Quanpin));
    assert(checked(msime::mac::scheme::Cantonese, false, true));
    assert(!checked(msime::mac::scheme::Cantonese, true, false));
    assert(checked(msime::mac::scheme::Quanpin, true, false));
    assert(!checked(msime::mac::scheme::Quanpin, false, true));
    assert(std::string(label(msime::mac::scheme::Cantonese)) == "简体输出");
    assert(std::string(label(msime::mac::scheme::Quanpin)) == "繁体输出");
    return 0;
}
