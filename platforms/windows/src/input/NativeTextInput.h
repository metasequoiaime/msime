#pragma once

#ifdef _WIN32
#include <Windows.h>

#include <string_view>
#include <vector>

namespace msimeui {

// Text panels are non-activating windows. Capture the editor that was in the
// foreground before the panel was opened and only inject while that same
// top-level window is still foreground. This keeps a late click from writing
// into an unrelated application after the user changes focus.
class NativeTextInputTarget final {
public:
    static NativeTextInputTarget Capture(HWND excluded = nullptr)
    {
        const HWND foreground = GetForegroundWindow();
        if (!foreground || (excluded && SameRoot(foreground, excluded)))
            return NativeTextInputTarget(nullptr);
        return NativeTextInputTarget(GetAncestor(foreground, GA_ROOT));
    }

    // 原有调用保留当前消息标记；自行记统计的面板显式传入 PanelTextSendInputExtraInfo。
    bool Insert(std::wstring_view text, HWND excluded = nullptr,
                ULONG_PTR extraInfo = static_cast<ULONG_PTR>(GetMessageExtraInfo())) const
    {
        constexpr size_t kMaxTextUnits = 4000;
        if (target_ == nullptr || text.empty() || text.size() > kMaxTextUnits ||
            !IsWindow(target_))
            return false;

        const HWND foreground = GetForegroundWindow();
        if (!foreground || (excluded && SameRoot(foreground, excluded)) ||
            !SameRoot(foreground, target_))
            return false;

        std::vector<INPUT> inputs;
        inputs.reserve(text.size() * 2);
        for (const wchar_t unit : text)
        {
            INPUT down{};
            down.type = INPUT_KEYBOARD;
            down.ki.wScan = static_cast<WORD>(unit);
            down.ki.dwFlags = KEYEVENTF_UNICODE;
            down.ki.dwExtraInfo = extraInfo;
            inputs.push_back(down);

            INPUT up = down;
            up.ki.dwFlags |= KEYEVENTF_KEYUP;
            inputs.push_back(up);
        }

        return SendInput(static_cast<UINT>(inputs.size()), inputs.data(), sizeof(INPUT)) ==
               static_cast<UINT>(inputs.size());
    }

    // 可编辑的面板只在自身仍为前台时恢复原编辑器；切到第三个窗口后不能抢回焦点。
    bool InsertFromPanel(std::wstring_view text, HWND panel,
                         ULONG_PTR extraInfo = static_cast<ULONG_PTR>(GetMessageExtraInfo())) const
    {
        if (!target_ || !panel || !IsWindow(target_) || text.empty() || text.size() > 4000)
            return false;
        const HWND foreground = GetForegroundWindow();
        if (foreground && SameRoot(foreground, panel)) {
            if (!SetForegroundWindow(target_))
                return false;
        }
        return Insert(text, panel, extraInfo);
    }

    bool valid() const noexcept { return target_ != nullptr; }

private:
    explicit NativeTextInputTarget(HWND target) : target_(target) {}

    static bool SameRoot(HWND left, HWND right) noexcept
    {
        return left != nullptr && right != nullptr &&
               GetAncestor(left, GA_ROOT) == GetAncestor(right, GA_ROOT);
    }

    HWND target_ = nullptr;
};

} // namespace msimeui
#endif
