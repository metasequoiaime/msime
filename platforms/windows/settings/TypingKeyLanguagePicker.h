#pragma once

#include "TypingKeyLanguagesModel.h"
#include <utility>
#include <winrt/Microsoft.UI.Xaml.Automation.h>
#include <winrt/Microsoft.UI.Xaml.Controls.h>

namespace msime::settings::typing {

template <class MakeLabel, class Choose>
void append_key_language_picker(
    winrt::Microsoft::UI::Xaml::Controls::StackPanel const &body,
    KeyLanguageSnapshot const &snapshot, std::string const &selection,
    MakeLabel const &label, Choose choose) {
  using namespace winrt::Microsoft::UI::Xaml;
  using namespace winrt::Microsoft::UI::Xaml::Controls;
  StackPanel row;
  row.Orientation(Orientation::Horizontal);
  row.Spacing(12);
  row.Margin(Thickness{0, 8, 0, 8});
  auto caption = label(L"语言");
  caption.VerticalAlignment(VerticalAlignment::Center);
  row.Children().Append(caption);
  ComboBox picker;
  picker.Width(168);
  picker.MinHeight(28);
  picker.FontSize(13);
  const auto choices = snapshot.choices();
  for (size_t index = 0; index < choices.size(); ++index) {
    picker.Items().Append(winrt::box_value(winrt::hstring(choices[index].title)));
    if (selection == choices[index].id)
      picker.SelectedIndex(static_cast<int32_t>(index));
  }
  Automation::AutomationProperties::SetName(picker, L"按键语言");
  Automation::AutomationProperties::SetAutomationId(picker, L"statisticsKeyLanguage");
  picker.SelectionChanged(
      [choices, choose = std::move(choose)](
          winrt::Windows::Foundation::IInspectable const &sender,
          SelectionChangedEventArgs const &) {
        const auto index = sender.as<ComboBox>().SelectedIndex();
        if (index >= 0 && static_cast<size_t>(index) < choices.size())
          choose(std::string(choices[static_cast<size_t>(index)].id));
      });
  row.Children().Append(picker);
  body.Children().Append(row);
}

} // namespace msime::settings::typing
