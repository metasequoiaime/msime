#include "SmartPunctuationForwardedKey.h"

#include <cassert>

int main()
{
    assert(Global::ForwardedSmartPunctuationContext(VK_3, L'3') == L'3');
    assert(Global::ForwardedSmartPunctuationContext(VK_3, L'\0') == L'3');
    assert(Global::ForwardedSmartPunctuationContext(VK_NUMPAD7, L'\0') == L'7');
    assert(Global::ForwardedSmartPunctuationContext(L'A', L'\0') == L'A');
    assert(Global::ForwardedSmartPunctuationContext(VK_3, L'中') == L'\0');
    return 0;
}
