#define NOMINMAX
#include <windows.h>
#include <uiautomation.h>
#include "AccessibleWindow.h"
#include <wrl/client.h>
#include <atomic>
#include <cstdio>
#include <stdexcept>
#include <string>
#include <thread>

using Microsoft::WRL::ComPtr;
using msime::windows::AccessibleWindow;
namespace {
void require_at(bool value, int line) { if (!value) throw std::runtime_error("UI Automation provider lifecycle failed at " + std::to_string(line)); }
#define require(value) require_at((value),__LINE__)
void check(HRESULT result) { require(SUCCEEDED(result)); }
LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
  if (message == WM_GETOBJECT) {
    const auto provider = reinterpret_cast<AccessibleWindow*>(GetWindowLongPtrW(window,GWLP_USERDATA));
    if (provider) if (const auto result = provider->answer(wparam,lparam)) return *result;
  }
  return DefWindowProcW(window,message,wparam,lparam);
}
void name(IUIAutomationElement* element, const wchar_t* expected) {
  BSTR value = nullptr; const auto result = element->get_CurrentName(&value);
  const bool matches = SUCCEEDED(result) && value && std::wstring(value,SysStringLen(value)) == expected;
  if (!matches) { std::fwprintf(stderr,L"UIA synthetic name mismatch: expected=%ls actual=%ls result=%08lx\n",expected,value ? value : L"",static_cast<unsigned long>(result)); }
  SysFreeString(value); require(matches);
}
}
int main() {
  try {
    check(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED));
    const auto instance = GetModuleHandleW(nullptr);
    WNDCLASSW descriptor{}; descriptor.hInstance = instance; descriptor.lpfnWndProc = procedure;
    descriptor.lpszClassName = L"MSIME.Synthetic.Accessibility.Lifecycle"; require(RegisterClassW(&descriptor) != 0);
    const auto window = CreateWindowW(descriptor.lpszClassName,L"Synthetic native host",WS_OVERLAPPEDWINDOW,
      40,40,300,180,nullptr,nullptr,instance,nullptr); require(window != nullptr);
    {
      AccessibleWindow provider(window);
      SetWindowLongPtrW(window,GWLP_USERDATA,reinterpret_cast<LONG_PTR>(&provider));
      msime::windows::AccessibleTree tree; tree.name = "Synthetic provider root";
      msime::windows::AccessibleElement row; row.id = 41; row.role = msime::windows::AccessibleRole::ListItem;
      row.name = "Synthetic candidate"; row.automation_id = "synthetic-candidate"; row.bounds = {0,0,180,40}; tree.elements.push_back(row);
      provider.publish(std::move(tree)); ShowWindow(window,SW_SHOWNOACTIVATE);
      std::atomic<int> phase{0}; std::atomic<bool> disconnected{false}; std::string failure;
      std::thread client([&] {
        try {
          check(CoInitializeEx(nullptr,COINIT_MULTITHREADED));
          {
            ComPtr<IUIAutomation> automation;
            check(CoCreateInstance(__uuidof(CUIAutomation),nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&automation)));
            ComPtr<IUIAutomationElement> root; check(automation->ElementFromHandle(window,&root)); require(root != nullptr);
            name(root.Get(),L"Synthetic provider root");
            VARIANT identifier{}; identifier.vt = VT_BSTR; identifier.bstrVal = SysAllocString(L"synthetic-candidate");
            require(identifier.bstrVal != nullptr);
            ComPtr<IUIAutomationCondition> condition;
            const auto predicate = automation->CreatePropertyCondition(UIA_AutomationIdPropertyId,identifier,&condition);
            VariantClear(&identifier); check(predicate);
            ComPtr<IUIAutomationElement> child; check(root->FindFirst(TreeScope_Children,condition.Get(),&child)); require(child != nullptr);
            name(child.Get(),L"Synthetic candidate"); phase.store(1);
            const auto deadline = GetTickCount64() + 10000;
            while (!disconnected.load()) { require(GetTickCount64() < deadline); Sleep(1); }
            // 保留客户端的旧元素引用，断开后必须实际返回不可用，不能继续读旧树。
            BSTR value = nullptr; const auto result = child->get_CurrentName(&value); SysFreeString(value);
            require(result == static_cast<HRESULT>(UIA_E_ELEMENTNOTAVAILABLE));
          }
          CoUninitialize();
        } catch (const std::exception& error) { failure = error.what(); }
        phase.store(2);
      });
      const auto deadline = GetTickCount64() + 20000;
      bool timed_out = false;
      while (phase.load() != 2) {
        MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
        if (phase.load() == 1 && !disconnected.load()) { provider.disconnect(); provider.disconnect(); disconnected.store(true); }
        if (GetTickCount64() >= deadline) { timed_out = true; disconnected.store(true); break; }
        MsgWaitForMultipleObjects(0,nullptr,FALSE,10,QS_ALLINPUT);
      }
      // CTest 的外层超时处理异常的 COM 阻塞；正常路径在消息泵仍活动时等待客户端退出。
      while (phase.load() != 2) {
        MSG message{}; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
        MsgWaitForMultipleObjects(0,nullptr,FALSE,10,QS_ALLINPUT);
      }
      client.join(); if (!failure.empty()) throw std::runtime_error(failure); require(!timed_out);
      require(!provider.answer(0,static_cast<LPARAM>(UiaRootObjectId)));
      SetWindowLongPtrW(window,GWLP_USERDATA,0);
    }
    DestroyWindow(window); UnregisterClassW(descriptor.lpszClassName,instance); CoUninitialize();
    std::puts("UI Automation: actual client root/child queries, retained-reference invalidation and repeated disconnect passed");
    return 0;
  } catch (const std::exception& error) { std::fprintf(stderr,"%s\n",error.what()); return 1; }
}
