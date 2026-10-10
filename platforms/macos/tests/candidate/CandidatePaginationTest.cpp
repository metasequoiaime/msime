#include "../../src/input/InputControllerKeyRouting.h"
#include "../../src/candidate/CandidateWheelRouting.h"
#include <stdexcept>

static void require(bool value, const char *message) { if (!value) throw std::runtime_error(message); }

int main() {
    using namespace metasequoia::mac;
    require(CandidatePageStart(0, 23, 9) == 0, "first page start");
    require(CandidatePageStart(8, 23, 9) == 0, "selected item stays on first page");
    require(CandidatePageStart(9, 23, 9) == 9, "second page start");
    require(CandidatePageStart(99, 23, 9) == 18, "out of range selection clamps to last page");
    require(CandidatePageEnd(9, 23, 9) == 17, "second page end");
    require(CandidatePageEnd(18, 23, 9) == 22, "short final page end");
    require(CandidatePageStart(0, 0, 9) == 0 && CandidatePageEnd(0, 0, 9) == 0, "empty page");
    require(ClassifyControllerKey(kVK_PageUp, true) == ControllerKeyAction::MoveCandidatePageUp, "page up key");
    require(ClassifyControllerKey(kVK_PageDown, true) == ControllerKeyAction::MoveCandidatePageDown, "page down key");
    require(ClassifyControllerKey(kVK_ANSI_LeftBracket, true, CandidatePageShortcut::Brackets, '[', false) == ControllerKeyAction::MoveCandidatePageUp, "bracket page up");
    require(ClassifyControllerKey(kVK_ANSI_RightBracket, true, CandidatePageShortcut::Brackets, ']', false) == ControllerKeyAction::MoveCandidatePageDown, "bracket page down");
    require(ClassifyControllerKey(kVK_ANSI_LeftBracket, true, CandidatePageShortcut::Brackets, '[', true) == ControllerKeyAction::Character, "modified bracket passthrough");
    require(ClassifyControllerKey(kVK_ANSI_LeftBracket, true, CandidatePageShortcut::Brackets, '[', false) == ControllerKeyAction::MoveCandidatePageUp, "physical bracket page up");
    require(ClassifyControllerKey(kVK_ANSI_Minus, true, CandidatePageShortcut::MinusEqual, '-', false, true) == ControllerKeyAction::Character, "Japanese minus passthrough");
    require(ClassifyControllerKey(kVK_ANSI_Equal, true, CandidatePageShortcut::MinusEqual, '+', false) == ControllerKeyAction::Character, "Unicode plus passthrough");
    require(ClassifyControllerKey(0, true, CandidatePageShortcut::Brackets, '[', false) == ControllerKeyAction::Character, "wrong physical bracket rejected");
    require(msime::mac::IsJapaneseMinusEqualInput(3, false, '-') && msime::mac::IsJapaneseMinusEqualInput(3, false, '='), "direct Japanese scheme punctuation");
    require(msime::mac::IsJapaneseMinusEqualInput(0, true, '-') && msime::mac::IsJapaneseMinusEqualInput(0, true, '='), "temporary Japanese punctuation");
    require(!msime::mac::IsJapaneseMinusEqualInput(0, false, '-') && !msime::mac::IsJapaneseMinusEqualInput(3, false, '['), "non-Japanese punctuation remains navigation");
    require(msime::mac::IsJapaneseMinusEqualKey(3, false, 27, 'x') &&
                msime::mac::IsJapaneseMinusEqualKey(3, false, 24, 'x'),
            "Japanese physical minus/equal keys bypass paging despite layout characters");
    require(msime::mac::IsJapaneseMinusEqualKey(0, true, 0, '-') &&
                !msime::mac::IsJapaneseMinusEqualKey(0, false, 27, '-'),
            "temporary Japanese keeps character fallback and ordinary input keeps paging");
    require(msime::mac::PhysicalCandidateDigitSlot(18) == 0 && msime::mac::PhysicalCandidateDigitSlot(25) == 8, "physical number row mapping");
    require(msime::mac::PhysicalCandidateDigitSlot(83) == 0 && msime::mac::PhysicalCandidateDigitSlot(92) == 8, "keypad digit mapping");
    require(msime::mac::PhysicalCandidateDigitSlot(29) == 9 && msime::mac::PhysicalCandidateDigitSlot(82) == 9, "the 0 keys name the tenth slot");
    require(msime::mac::PhysicalCandidateDigitSlot(0) == -1 && msime::mac::PhysicalCandidateDigitSlot(27) == -1, "non-candidate key codes rejected");
    require(msime::mac::CandidateDigitSlotOnPage(9, 10) == 9, "0 picks the tenth candidate on a page of ten");
    require(msime::mac::CandidateDigitSlotOnPage(9, 9) == -1 && msime::mac::CandidateDigitSlotOnPage(9, 6) == -1,
            "below ten a page has no tenth slot, so 0 stays an ordinary key");
    require(msime::mac::CandidateDigitSlotOnPage(8, 6) == 8 && msime::mac::CandidateDigitSlotOnPage(-1, 10) == -1,
            "1-9 and non-digits are unchanged");
    require(msime::mac::ShouldRoutePhysicalCandidateDigit(true, false, false, false), "ordinary candidate digits route");
    require(!msime::mac::ShouldRoutePhysicalCandidateDigit(true, false, true, false), "digits a mode spells with reach the engine");
    require(!msime::mac::ShouldRoutePhysicalCandidateDigit(true, true, false, false), "nine-key digits reach the engine");
    require(!msime::mac::ShouldRoutePhysicalCandidateDigit(true, false, false, true), "modified digits reach the engine");
    require(!msime::mac::ShouldRoutePhysicalCandidateDigit(false, false, false, false), "hidden candidate panel does not route digits");
    require(msime::mac::ShouldRouteSpellingShiftCandidateDigit(true, true, true, false),
            "a mode spelling with digits selects with Shift and a digit, which its input cannot use");
    require(!msime::mac::ShouldRouteSpellingShiftCandidateDigit(true, true, false, false),
            "an unshifted digit is still input");
    require(!msime::mac::ShouldRouteSpellingShiftCandidateDigit(true, false, true, false),
            "outside such a mode a shifted digit is punctuation, not a selection");
    require(!msime::mac::ShouldRouteSpellingShiftCandidateDigit(false, true, true, false),
            "with no candidate panel there is nothing to select");
    require(!msime::mac::ShouldRouteSpellingShiftCandidateDigit(true, true, true, true),
            "a shifted digit the mode spells with, such as ( in expression mode, stays input");
    require(msime::mac::PhysicalCandidateDigitCharacter(0) == '1' && msime::mac::PhysicalCandidateDigitCharacter(8) == '9' &&
                msime::mac::PhysicalCandidateDigitCharacter(-1) == '\0' && msime::mac::PhysicalCandidateDigitCharacter(9) == '0' &&
                msime::mac::PhysicalCandidateDigitCharacter(10) == '\0',
            "candidate slots name the digit their key types");
    require(msime::mac::PhysicalKeySoundClass(49) == 1 && msime::mac::PhysicalKeySoundClass(36) == 2 &&
                msime::mac::PhysicalKeySoundClass(76) == 2 && msime::mac::PhysicalKeySoundClass(51) == 3,
            "space, both enter keys and backspace have their own key sound class");
    require(msime::mac::PhysicalKeySoundClass(0) == 0 && msime::mac::PhysicalKeySoundClass(117) == 0 &&
                msime::mac::PhysicalKeySoundClass(18) == 0,
            "letters, digits and forward delete use the default key sound");
    require(msime::mac::IsKeypadDecimal(65) && !msime::mac::IsKeypadDecimal(0), "keypad decimal mapping");
    require(msime::mac::KeypadPunctuation(65) == '.' && msime::mac::KeypadPunctuation(67) == '*' &&
                msime::mac::KeypadPunctuation(69) == '+' && msime::mac::KeypadPunctuation(75) == '/' &&
                msime::mac::KeypadPunctuation(78) == '-' && msime::mac::KeypadPunctuation(81) == '=' &&
                msime::mac::KeypadPunctuation(95) == ',', "keypad punctuation mapping");
    require(msime::mac::KeypadPunctuation(82) == '\0' && msime::mac::KeypadPunctuation(0) == '\0',
            "non-punctuation keypad codes rejected");
    using msime::mac::CandidateWheelAction;
    require(msime::mac::CandidateWheelPageAction(1, true, true, false) == CandidateWheelAction::PreviousPage, "wheel previous page");
    require(msime::mac::CandidateWheelPageAction(-1, true, false, true) == CandidateWheelAction::NextPage, "wheel next page");
    require(msime::mac::CandidateWheelPageAction(-1, false, true, true) == CandidateWheelAction::None, "disabled wheel passthrough");
    require(msime::mac::CandidateWheelPageAction(1, true, false, true) == CandidateWheelAction::None, "wheel respects first page");
    require(msime::mac::CandidateWheelPageAction(-1, true, true, false) == CandidateWheelAction::None, "wheel respects last page");
    using msime::mac::ConsumeCandidateWheelDelta;
    double wheel = 0.0;
    int wheelPages = 0;
    for (int event = 0; event < 20; ++event)
        wheelPages += ConsumeCandidateWheelDelta(wheel, -3.0, true, event == 0, false, 40.0);
    require(wheelPages == -1 && wheel == -20.0, "one short trackpad swipe pages once and keeps the remainder");
    require(ConsumeCandidateWheelDelta(wheel, -100.0, true, false, true, 40.0) == 0 && wheel == 0.0,
            "momentum scrolling never pages and drops the remainder");
    wheel = -30.0;
    require(ConsumeCandidateWheelDelta(wheel, 30.0, true, false, false, 40.0) == 0 && wheel == 30.0,
            "direction reversal drops the old remainder");
    require(ConsumeCandidateWheelDelta(wheel, 10.0, true, false, false, 40.0) == 1 && wheel == 0.0,
            "precise scrolling pages once per full notch toward the previous page");
    wheel = 30.0;
    require(ConsumeCandidateWheelDelta(wheel, 5.0, true, true, false, 40.0) == 0 && wheel == 5.0,
            "gesture begin starts from an empty accumulator");
    wheel = 0.0;
    require(ConsumeCandidateWheelDelta(wheel, -130.0, true, false, false, 40.0) == -3 && wheel == -10.0,
            "a multi-notch precise delta splits into several pages");
    wheel = 25.0;
    require(ConsumeCandidateWheelDelta(wheel, -6.0, false, false, false, 40.0) == -1 && wheel == 0.0,
            "classic wheel pages once per notch regardless of line acceleration");
    require(ConsumeCandidateWheelDelta(wheel, 0.0, false, false, false, 40.0) == 0, "zero classic delta does not page");
    static_assert(msime::mac::CandidateWheelPreciseNotch > 0.0);
    static_assert(msime::mac::KoreanKeyLetter('r', false) == 'r' && msime::mac::KoreanKeyLetter('R', false) == 'r',
                  "Caps Lock must not turn a Korean key into its shifted jamo");
    static_assert(msime::mac::KoreanKeyLetter('r', true) == 'R' && msime::mac::KoreanKeyLetter('R', true) == 'R',
                  "Shift decides the case of a Korean key");
    static_assert(msime::mac::KoreanKeyLetter('1', true) == '1' && msime::mac::KoreanKeyLetter('.', false) == '.' &&
                      msime::mac::KoreanKeyLetter(' ', true) == ' ',
                  "Only letters are recased for Korean");
    require(msime::mac::JapaneseSpaceCommitsFallback(1, msime::mac::CandidateSourceFallback),
            "a lone Fallback row (bare Shift+R) is committed by Japanese Space instead of arming a conversion");
    require(!msime::mac::JapaneseSpaceCommitsFallback(1, 0) &&
                !msime::mac::JapaneseSpaceCommitsFallback(2, msime::mac::CandidateSourceFallback) &&
                !msime::mac::JapaneseSpaceCommitsFallback(0, msime::mac::CandidateSourceFallback),
            "Japanese Space still arms a conversion for real candidates");
}
