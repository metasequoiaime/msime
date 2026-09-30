#pragma once

#include "VoiceTheme.h"
#include <windows.h>
#include <atomic>
#include <functional>
#include <mutex>
#include <string>

class WaveOverlay
{
  public:
    enum class CompactStatus
    {
        None,
        Recognizing,
        Processing,
    };

    enum class Action
    {
        Cancel,
        Confirm,
    };

    WaveOverlay();
    ~WaveOverlay();

    WaveOverlay(const WaveOverlay &) = delete;
    WaveOverlay &operator=(const WaveOverlay &) = delete;

    bool init(HINSTANCE instance, std::function<void(Action)> action_handler);
    void shutdown();

    void show();
    void hide();

    void set_listening(bool listening);
    void set_input_level(float level);
    // The theme palette in the overlay's own light/dark mode; see voice_overlay_colors.
    void set_palette(const msime::windows::CandidatePalette &palette);
    void set_transcript(const std::wstring &text);
    // When false, the overlay stays compact and never paints live ASR text.
    void set_show_transcript(bool show);
    void set_compact_status(CompactStatus status);
    void set_actions_visible(bool visible);
    HWND handle() const { return hwnd_; }

  private:
    static LRESULT CALLBACK wnd_proc(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);

    LRESULT handle_message(HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);
    bool ensure_render_target();
    bool ensure_text_layout(const std::wstring &transcript, float width, float height);
    void release_render_target();
    void release_text_layout();
    void update_wave_levels();
    void draw();
    // Adopt a monitor's DPI for the window size, the hit tests and the render
    // target, all of which must agree on one value.
    void apply_dpi(UINT dpi);
    void update_dpi_scale();
    void update_window_bounds();
    bool hit_test_action(float x, float y, Action &action) const;

  private:
    static constexpr int kLevelCount = 12;

    HINSTANCE instance_ = nullptr;
    HWND hwnd_ = nullptr;
    UINT dpi_ = 96;
    float scale_x_ = 1.0f;
    float scale_y_ = 1.0f;

    std::atomic<bool> listening_{false};
    std::atomic<float> input_level_{0.0f};
    std::atomic<bool> show_transcript_{true};
    std::atomic<CompactStatus> compact_status_{CompactStatus::None};
    std::atomic<bool> actions_visible_{false};
    Action pressed_action_ = Action::Confirm;
    bool action_pressed_ = false;
    std::function<void(Action)> action_handler_;
    std::mutex transcript_mutex_;
    std::wstring transcript_;
    float levels_[kLevelCount] = {};
    float amplitudes_[kLevelCount] = {};
    float phases_[kLevelCount] = {};
    float freqs_[kLevelCount] = {};
    msime::windows::VoiceOverlayColors colors_ =
        msime::windows::voice_overlay_colors(msime::windows::candidate_native_palette(true));

    struct ID2D1Factory *factory_ = nullptr;
    struct ID2D1HwndRenderTarget *render_target_ = nullptr;
    struct ID2D1SolidColorBrush *bar_brush_ = nullptr;
    struct ID2D1SolidColorBrush *text_brush_ = nullptr;
    struct ID2D1SolidColorBrush *bg_brush_ = nullptr;
    struct ID2D1SolidColorBrush *border_brush_ = nullptr;
    struct ID2D1SolidColorBrush *action_bg_brush_ = nullptr;
    struct IDWriteFactory *write_factory_ = nullptr;
    struct IDWriteTextFormat *text_format_ = nullptr;
    struct IDWriteTextFormat *processing_text_format_ = nullptr;
    struct IDWriteTextLayout *text_layout_ = nullptr;
    std::wstring layout_source_transcript_;
};
