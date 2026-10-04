#include "tests/includes/test_framework.h"

#include "msimeui/DeviceResources.h"

#include <cmath>
#include <cstdio>

namespace
{
class HiddenWindow
{
  public:
    HiddenWindow()
    {
        REQUIRE(SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)));
        hwnd = CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP, L"STATIC", L"DPI resource test", WS_POPUP, 0, 0, 256, 128,
                               nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
        REQUIRE(hwnd != nullptr);
    }

    ~HiddenWindow()
    {
        DestroyWindow(hwnd);
        CoUninitialize();
    }

    HWND hwnd = nullptr;
};

void CheckReusedTargetDpi(bool composition)
{
    HiddenWindow window;
    msimeui::DeviceResources resources;
    const auto ensure = [&]() {
        return composition ? resources.EnsureForComposition(window.hwnd) : resources.EnsureForWindow(window.hwnd);
    };
    REQUIRE(ensure());
    ID2D1RenderTarget *const target = resources.GetRenderTarget();
    REQUIRE(target != nullptr);
    const float windowDpi = static_cast<float>(GetDpiForWindow(window.hwnd));

    // Reproduce the retained render target from the previous monitor DPI without
    // changing the user's display settings. Reuse must refresh DPI, not recreate it.
    for (const float previousDpi : {windowDpi / 2.0f, 96.0f, 120.0f, 144.0f, 192.0f, windowDpi * 2.0f})
    {
        target->SetDpi(previousDpi, previousDpi);
        REQUIRE(ensure());
        REQUIRE(resources.GetRenderTarget() == target);
        float dpiX = 0.0f;
        float dpiY = 0.0f;
        target->GetDpi(&dpiX, &dpiY);
        std::printf("%s retained DPI %.0f -> window %.0f, target %.0f/%.0f\n", composition ? "composition" : "HWND",
                    previousDpi, windowDpi, dpiX, dpiY);
        REQUIRE_NEAR(dpiX, windowDpi);
        REQUIRE_NEAR(dpiY, windowDpi);
        const D2D1_SIZE_F size = target->GetSize();
        const D2D1_SIZE_U pixels = target->GetPixelSize();
        REQUIRE_NEAR(size.width, pixels.width * 96.0f / windowDpi);
        REQUIRE_NEAR(size.height, pixels.height * 96.0f / windowDpi);
    }

    // A smaller client area also reuses the larger composition surface.
    REQUIRE(SetWindowPos(window.hwnd, nullptr, 0, 0, 128, 64, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE));
    target->SetDpi(windowDpi * 1.25f, windowDpi * 1.25f);
    REQUIRE(ensure());
    REQUIRE(resources.GetRenderTarget() == target);
    float dpiX = 0.0f;
    float dpiY = 0.0f;
    target->GetDpi(&dpiX, &dpiY);
    REQUIRE_NEAR(dpiX, windowDpi);
    REQUIRE_NEAR(dpiY, windowDpi);
}
} // namespace

TEST_CASE(hwnd_target_refreshes_dpi_when_reused)
{
    CheckReusedTargetDpi(false);
}

TEST_CASE(composition_target_refreshes_dpi_when_reused)
{
    CheckReusedTargetDpi(true);
}

TEST_CASE(text_format_falls_back_for_an_empty_family)
{
    HiddenWindow window;
    msimeui::DeviceResources resources;
    REQUIRE(resources.EnsureFactories());
    REQUIRE(resources.GetTextFormat(L"", 16.0f, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING,
                                    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_WORD_WRAPPING_NO_WRAP) != nullptr);
}
