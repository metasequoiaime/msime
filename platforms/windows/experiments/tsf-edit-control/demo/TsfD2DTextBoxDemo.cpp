#include <Windows.h>
#include <shellapi.h>
#include <string>
#include <cwchar>
#include "../../../compat/WindowsDpi.h"

#include "TsfD2DTextBox.h"
#include "TsfTextServices.h"
#include "msime_edition.h"

// 旧版 MinGW 缺少此宏；取 Windows SDK msctf.h 中的同一进程作用域标志。
#ifndef TF_IPPMF_FORPROCESS
#define TF_IPPMF_FORPROCESS 0x10000000
#endif

namespace
{
const wchar_t *kWindowClass = L"TsfD2DTextBoxDemoWindow";
const wchar_t *kWindowTitle = L"TsfD2DTextBox Demo";

constexpr int kOuterMargin = 20;
constexpr int kSectionGap = 16;
constexpr int kSearchTitleHeight = 24;
constexpr int kRoundedBoxHeight = 68;
constexpr int kRoundedInset = 10;
constexpr int kRoundedRadius = 18;

TsfD2DTextBox *g_editorTextBox = nullptr;
TsfD2DTextBox *g_searchTextBox = nullptr;
RECT g_searchOuterRect = {};
ITfInputProcessorProfileMgr *g_profiles = nullptr;
const wchar_t *g_tipStage = L"create profile manager";
constexpr CLSID kService = MSIME_EDITION_CLSID;
constexpr GUID kProfile = MSIME_EDITION_PROFILE_GUID;

// 验收必须先通过系统注册本次构建；这里仅选择当前进程的真实 TSF 输入源。
HRESULT ActivateRegisteredTip()
{
    HRESULT result = S_OK;
    if (!g_profiles)
        result = CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
            IID_ITfInputProcessorProfileMgr, reinterpret_cast<void **>(&g_profiles));
    if (FAILED(result))
        return result;
    g_tipStage = L"activate registered profile";
    result = g_profiles->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR, MSIME_EDITION_LANGID,
        kService, kProfile, nullptr, TF_IPPMF_FORPROCESS);
    if (result != S_OK)
        return FAILED(result) ? result : E_FAIL;
    g_tipStage = L"verify active TSF profile";
    TF_INPUTPROCESSORPROFILE active{};
    const ULONGLONG deadline = GetTickCount64() + 2000;
    do
    {
        MSG pending{};
        while (PeekMessageW(&pending, nullptr, 0, 0, PM_REMOVE))
        {
            TranslateMessage(&pending);
            DispatchMessageW(&pending);
        }
        result = g_profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &active);
        if (result == S_OK && IsEqualCLSID(active.clsid, kService) && IsEqualGUID(active.guidProfile, kProfile))
            return S_OK;
        MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
    } while (GetTickCount64() < deadline);
    return FAILED(result) ? result : E_FAIL;
}

RECT MakeRect(int left, int top, int right, int bottom)
{
    RECT rc = {left, top, right, bottom};
    return rc;
}

void LayoutControls(HWND hwnd)
{
    RECT rcClient = {};
    GetClientRect(hwnd, &rcClient);

    const int width = rcClient.right - rcClient.left;
    const int height = rcClient.bottom - rcClient.top;

    g_searchOuterRect = MakeRect(kOuterMargin, height - kOuterMargin - kRoundedBoxHeight, width - kOuterMargin,
                                 height - kOuterMargin);

    RECT editorBounds = MakeRect(kOuterMargin, kOuterMargin, width - kOuterMargin,
                                 g_searchOuterRect.top - kSectionGap - kSearchTitleHeight);
    RECT searchBounds = MakeRect(g_searchOuterRect.left + kRoundedInset, g_searchOuterRect.top + kRoundedInset,
                                 g_searchOuterRect.right - kRoundedInset, g_searchOuterRect.bottom - kRoundedInset);

    if (g_editorTextBox)
    {
        g_editorTextBox->Move(editorBounds.left, editorBounds.top, editorBounds.right - editorBounds.left,
                              editorBounds.bottom - editorBounds.top);
    }

    if (g_searchTextBox)
    {
        g_searchTextBox->Move(searchBounds.left, searchBounds.top, searchBounds.right - searchBounds.left,
                              searchBounds.bottom - searchBounds.top);
    }
}

void PaintDemoBackground(HWND hwnd)
{
    PAINTSTRUCT ps = {};
    HDC hdc = BeginPaint(hwnd, &ps);

    RECT rcClient = {};
    GetClientRect(hwnd, &rcClient);

    HBRUSH backgroundBrush = CreateSolidBrush(RGB(245, 246, 248));
    FillRect(hdc, &rcClient, backgroundBrush);
    DeleteObject(backgroundBrush);

    HBRUSH roundedBrush = CreateSolidBrush(RGB(255, 255, 255));
    HPEN roundedPen = CreatePen(PS_SOLID, 1, RGB(210, 214, 220));
    HGDIOBJ oldBrush = SelectObject(hdc, roundedBrush);
    HGDIOBJ oldPen = SelectObject(hdc, roundedPen);
    RoundRect(hdc, g_searchOuterRect.left, g_searchOuterRect.top, g_searchOuterRect.right, g_searchOuterRect.bottom,
              kRoundedRadius, kRoundedRadius);
    SelectObject(hdc, oldPen);
    SelectObject(hdc, oldBrush);
    DeleteObject(roundedPen);
    DeleteObject(roundedBrush);

    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, RGB(104, 112, 124));
    TextOutW(hdc, g_searchOuterRect.left + 14, g_searchOuterRect.top - kSearchTitleHeight,
             L"Rounded Single-Line Text Box", 29);

    EndPaint(hwnd, &ps);
}

LRESULT CALLBACK DemoWndProc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam)
{
    switch (message)
    {
    case WM_CREATE:
    {
        g_editorTextBox = new TsfD2DTextBox();
        g_searchTextBox = new TsfD2DTextBox();

        RECT emptyBounds = MakeRect(0, 0, 0, 0);
        g_editorTextBox->Create(GetModuleHandle(NULL), hwnd, emptyBounds);
        g_searchTextBox->Create(GetModuleHandle(NULL), hwnd, emptyBounds);
        LayoutControls(hwnd);
        return 0;
    }

    case WM_SHOWWINDOW:
        if (wParam && g_editorTextBox && g_editorTextBox->GetHwnd())
        {
            SetFocus(g_editorTextBox->GetHwnd());
        }
        return 0;

    case WM_SETFOCUS:
        if (g_editorTextBox && g_editorTextBox->GetHwnd())
        {
            SetFocus(g_editorTextBox->GetHwnd());
        }
        return 0;

    case WM_SIZE:
        LayoutControls(hwnd);
        InvalidateRect(hwnd, NULL, TRUE);
        return 0;

    case WM_PAINT:
        PaintDemoBackground(hwnd);
        return 0;

    case WM_DESTROY:
        if (g_profiles)
        {
            g_profiles->Release();
            g_profiles = nullptr;
        }
        delete g_editorTextBox;
        delete g_searchTextBox;
        g_editorTextBox = nullptr;
        g_searchTextBox = nullptr;
        PostQuitMessage(0);
        return 0;

    default:
        return DefWindowProc(hwnd, message, wParam, lParam);
    }
}
} // namespace

int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE, PWSTR, int nCmdShow)
{
    msime::windows::compat::process_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
    CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);

    TsfD2DTextBox::RegisterClass(hInstance);

    WNDCLASSEX wcex = {};
    wcex.cbSize = sizeof(WNDCLASSEX);
    wcex.lpfnWndProc = DemoWndProc;
    wcex.hInstance = hInstance;
    wcex.hCursor = LoadCursor(NULL, IDC_ARROW);
    wcex.hbrBackground = reinterpret_cast<HBRUSH>(COLOR_WINDOW + 1);
    wcex.lpszClassName = kWindowClass;
    RegisterClassEx(&wcex);

    HWND hwnd = CreateWindow(kWindowClass, kWindowTitle, WS_OVERLAPPEDWINDOW | WS_VISIBLE, CW_USEDEFAULT, CW_USEDEFAULT,
                             960, 640, NULL, NULL, hInstance, NULL);
    ShowWindow(hwnd, nCmdShow);
    UpdateWindow(hwnd);

    int argumentCount = 0;
    LPWSTR *arguments = CommandLineToArgvW(GetCommandLineW(), &argumentCount);
    if (arguments && argumentCount > 1)
    {
        const HRESULT activated = argumentCount == 2 && std::wstring(arguments[1]) == L"--activate-profile"
            ? ActivateRegisteredTip() : E_INVALIDARG;
        LocalFree(arguments);
        if (FAILED(activated))
        {
            wchar_t error[192]{};
            swprintf_s(error, L"%s failed: 0x%08lX", g_tipStage, static_cast<unsigned long>(activated));
            MessageBoxW(hwnd, error, L"TSF acceptance", MB_OK | MB_ICONERROR);
            DestroyWindow(hwnd);
            CoUninitialize();
            return 1;
        }
        SetWindowTextW(hwnd, L"TSF acceptance - registered production TIP");
    }
    else if (arguments)
        LocalFree(arguments);

    ITfKeystrokeMgr *keystrokes = nullptr;
    if (g_pThreadMgr)
        g_pThreadMgr->QueryInterface(IID_ITfKeystrokeMgr, reinterpret_cast<void **>(&keystrokes));
    MSG msg = {};
    while (GetMessage(&msg, NULL, 0, 0))
    {
        // 验收窗口切到前台后可用 F12 重新选择测试版，避免系统恢复该应用原来的输入源。
        if (g_profiles && msg.message == WM_KEYDOWN && msg.wParam == VK_F12)
        {
            const HRESULT activated = ActivateRegisteredTip();
            SetWindowTextW(hwnd, SUCCEEDED(activated)
                ? L"TSF acceptance - registered production TIP"
                : L"TSF acceptance - profile activation failed");
            continue;
        }
        // 自绘文本控件必须先把键消息交给 TSF；直接 TranslateMessage 会绕过 TIP。
        BOOL eaten = FALSE;
        if (keystrokes && (msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN) &&
            SUCCEEDED(keystrokes->TestKeyDown(msg.wParam, msg.lParam, &eaten)) && eaten &&
            SUCCEEDED(keystrokes->KeyDown(msg.wParam, msg.lParam, &eaten)) && eaten)
            continue;
        if (keystrokes && (msg.message == WM_KEYUP || msg.message == WM_SYSKEYUP) &&
            SUCCEEDED(keystrokes->TestKeyUp(msg.wParam, msg.lParam, &eaten)) && eaten &&
            SUCCEEDED(keystrokes->KeyUp(msg.wParam, msg.lParam, &eaten)) && eaten)
            continue;
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }
    if (keystrokes)
        keystrokes->Release();

    CoUninitialize();
    return static_cast<int>(msg.wParam);
}
