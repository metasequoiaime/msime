#pragma once

#include "../TypingKeyLanguagesModel.h"
#include <windows.h>
#include <commctrl.h>

namespace msime::settings::win7 {

class StatisticsKeyLanguagePicker {
public:
  void update(HWND control, typing::KeyLanguageSnapshot const &snapshot) {
    selection_ = snapshot.resolved(selection_);
    choices_ = snapshot.choices();
    SendMessageW(control, CB_RESETCONTENT, 0, 0);
    for (size_t index = 0; index < choices_.size(); ++index) {
      SendMessageW(control, CB_ADDSTRING, 0,
                   reinterpret_cast<LPARAM>(choices_[index].title));
      if (selection_ == choices_[index].id)
        SendMessageW(control, CB_SETCURSEL, index, 0);
    }
  }

  void choose(HWND control) {
    const auto index = SendMessageW(control, CB_GETCURSEL, 0, 0);
    if (index >= 0 && static_cast<size_t>(index) < choices_.size())
      selection_ = choices_[static_cast<size_t>(index)].id;
  }

  std::string const &selection() const { return selection_; }

private:
  std::string selection_ = "all";
  std::vector<typing::KeyLanguageChoice> choices_;
};

} // namespace msime::settings::win7
