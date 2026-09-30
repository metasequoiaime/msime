#include "../src/overlay/WaveOverlayPlacement.h"
#include "../src/overlay/ModeBadgeStyle.h"

#include <cassert>
#include <optional>
#include <vector>

int main() {
  using msime::linux_host::wave_overlay_bottom_center;
  using msime::linux_host::WaveOverlayWorkArea;

  const auto primary = wave_overlay_bottom_center({0, 0, 1920, 1080}, 420, 132);
  assert(primary.x == 750);
  assert(primary.y == 924);

  const auto negative =
      wave_overlay_bottom_center({-1920, 40, 1920, 1040}, 420, 132);
  assert(negative.x == -1170);
  assert(negative.y == 924);

  const auto narrow =
      wave_overlay_bottom_center({100, 200, 300, 100}, 420, 132);
  assert(narrow.x == 100);
  assert(narrow.y == 200);

  const auto custom_margin = wave_overlay_bottom_center(
      WaveOverlayWorkArea{10, -20, 800, 600}, 400, 100, 16);
  assert(custom_margin.x == 210);
  assert(custom_margin.y == 464);

  using msime::linux_host::wave_overlay_monitor_bottom_center;
  using msime::linux_host::wave_overlay_monitor_work;
  using msime::linux_host::wave_overlay_pick_monitor;
  using msime::linux_host::wave_overlay_scale;
  using msime::linux_host::wave_overlay_scaled;
  using msime::linux_host::WaveOverlayMonitor;
  using msime::linux_host::WaveOverlayPosition;

  // Monitor B sits left of A at a negative x; the focused window's centre decides, not the pointer or the primary flag.
  const std::vector<WaveOverlayMonitor> side_by_side = {
      {{0, 0, 1920, 1080}, {0, 0, 1920, 1080}},
      {{-2560, 0, 2560, 1440}, {-2560, 0, 2560, 1440}}};
  const auto focused = wave_overlay_pick_monitor(
      side_by_side, WaveOverlayPosition{-1280, 700},
      WaveOverlayPosition{100, 100}, std::size_t{0});
  assert(focused && *focused == 1);

  // A focus centre outside every monitor (an off-screen window) falls through to the pointer.
  const auto by_pointer = wave_overlay_pick_monitor(
      side_by_side, WaveOverlayPosition{-9000, 0},
      WaveOverlayPosition{-1, 1439}, std::size_t{0});
  assert(by_pointer && *by_pointer == 1);
  // The 1080 px monitor A next to the 1440 px monitor B leaves a dead zone under A; a focus centre there also falls through to the pointer on B instead of the primary A.
  const auto dead_zone = wave_overlay_pick_monitor(
      side_by_side, WaveOverlayPosition{960, 1200},
      WaveOverlayPosition{-1280, 700}, std::size_t{0});
  assert(dead_zone && *dead_zone == 1);
  const auto pointer_only = wave_overlay_pick_monitor(
      side_by_side, std::nullopt, WaveOverlayPosition{1919, 0}, std::size_t{1});
  assert(pointer_only && *pointer_only == 0);

  const auto by_primary = wave_overlay_pick_monitor(
      side_by_side, std::nullopt, std::nullopt, std::size_t{1});
  assert(by_primary && *by_primary == 1);
  const auto first = wave_overlay_pick_monitor(side_by_side, std::nullopt,
                                               std::nullopt, std::nullopt);
  assert(first && *first == 0);
  const auto stale_primary = wave_overlay_pick_monitor(
      side_by_side, std::nullopt, std::nullopt, std::size_t{5});
  assert(stale_primary && *stale_primary == 0);
  assert(!wave_overlay_pick_monitor({}, WaveOverlayPosition{0, 0},
                                    std::nullopt, std::nullopt));

  // _NET_WORKAREA is one rectangle across both monitors; a 48 px bottom panel on the 1080 px monitor cuts it to 1032 px, and the intersection keeps the bar above that panel.
  const auto panel_work =
      wave_overlay_monitor_work({0, 0, 1920, 1080}, {{-2560, 0, 4480, 1032}});
  assert(panel_work.x == 0 && panel_work.y == 0);
  assert(panel_work.width == 1920 && panel_work.height == 1032);
  const WaveOverlayMonitor panel_monitor{{0, 0, 1920, 1080}, panel_work};
  const auto above_panel =
      wave_overlay_monitor_bottom_center(panel_monitor, 420, 132, 10);
  assert(above_panel.x == 750);
  assert(above_panel.y == 1032 - 132 - 10);

  // Mutter's per-monitor list: the rectangle overlapping the monitor most is that monitor's work area.
  const auto gtk_work = wave_overlay_monitor_work(
      {-2560, 0, 2560, 1440}, {{0, 0, 1920, 1032}, {-2560, 32, 2560, 1408}});
  assert(gtk_work.x == -2560 && gtk_work.y == 32);
  assert(gtk_work.width == 2560 && gtk_work.height == 1408);

  // A work area that misses the monitor entirely leaves the whole monitor.
  const auto disjoint =
      wave_overlay_monitor_work({0, 0, 1920, 1080}, {{3000, 0, 100, 100}});
  assert(disjoint.x == 0 && disjoint.width == 1920 && disjoint.height == 1080);
  const auto none = wave_overlay_monitor_work({5, 6, 7, 8}, {});
  assert(none.x == 5 && none.y == 6 && none.width == 7 && none.height == 8);

  // A left dock narrows the work area, but the bar still centres on the whole monitor as on Windows.
  const WaveOverlayMonitor docked{{0, 0, 1920, 1080}, {72, 0, 1848, 1080}};
  const auto docked_position =
      wave_overlay_monitor_bottom_center(docked, 420, 132, 10);
  assert(docked_position.x == 750);
  assert(docked_position.y == 938);

  assert(wave_overlay_scale(std::nullopt, std::nullopt) == 1.0);
  assert(wave_overlay_scale(144.0, std::nullopt) == 1.5);
  assert(wave_overlay_scale(192.0, 1.0) == 1.0);
  assert(wave_overlay_scale(96.0, 2.0) == 2.0);
  assert(wave_overlay_scale(48.0, std::nullopt) == 1.0);
  assert(wave_overlay_scale(960.0, std::nullopt) == 4.0);
  assert(wave_overlay_scale(std::nullopt, 0.0) == 1.0);
  assert(wave_overlay_scale(120.0, -2.0) == 1.25);
  assert(wave_overlay_scaled(14, 1.5) == 21);
  assert(wave_overlay_scaled(3, 1.5) == 5);

  // At a scale of 2 the bar doubles and stays centred on the monitor, 20 device pixels above the work area bottom.
  const auto scale = wave_overlay_scale(std::nullopt, 2.0);
  const auto width = wave_overlay_scaled(420, scale);
  const auto height = wave_overlay_scaled(132, scale);
  assert(width == 840 && height == 264);
  const WaveOverlayMonitor hidpi{{-3840, 0, 3840, 2160},
                                 {-3840, 0, 3840, 2100}};
  const auto hidpi_position = wave_overlay_monitor_bottom_center(
      hidpi, width, height, wave_overlay_scaled(10, scale));
  assert(hidpi_position.x == -3840 + (3840 - 840) / 2);
  assert(hidpi_position.x + width / 2 == -1920);
  assert(hidpi_position.y == 2100 - 264 - 20);

  // A bar larger than the monitor aligns to its origin rather than spilling onto the neighbour.
  const WaveOverlayMonitor tiny{{-800, 100, 800, 200}, {-800, 120, 800, 150}};
  const auto oversized = wave_overlay_monitor_bottom_center(tiny, 840, 264, 20);
  assert(oversized.x == -800);
  assert(oversized.y == 120);

  // The Fcitx5 X11 mode badge at the default 24 px, 100 % toolbar size with a 22 px glyph: 74 x 44 logical pixels, 24 in from the right and bottom of the work area of the monitor holding focus. Monitor A is 1920 x 1080 at the origin with a 48 px bottom panel; monitor B is a 2560 x 1440 display to its left with a 64 px dock on its right edge; the work areas come from Mutter's per-monitor list.
  {
    using msime::linux_host::wave_overlay_bottom_right;
    const auto badge = msime::linux_host::mode_badge_layout(msime::linux_host::ModeBadgeMetrics{}, 22.0, true);
    assert(badge.width == 74 && badge.height == 44);
    const std::vector<WaveOverlayWorkArea> badge_work_areas = {
        {0, 0, 1920, 1032}, {-2560, 0, 2496, 1440}};
    std::vector<WaveOverlayMonitor> badge_monitors = {
        {{0, 0, 1920, 1080}, {0, 0, 1920, 1080}},
        {{-2560, 0, 2560, 1440}, {-2560, 0, 2560, 1440}}};
    for (auto &monitor : badge_monitors)
      monitor.work = wave_overlay_monitor_work(monitor.full, badge_work_areas);
    const auto badge_at = [&](std::optional<WaveOverlayPosition> focus,
                              std::optional<WaveOverlayPosition> pointer,
                              double badge_scale) {
      const auto index = wave_overlay_pick_monitor(badge_monitors, focus,
                                                   pointer, std::size_t{0});
      assert(index);
      return wave_overlay_bottom_right(
          badge_monitors[*index].work, wave_overlay_scaled(badge.width, badge_scale),
          wave_overlay_scaled(badge.height, badge_scale),
          wave_overlay_scaled(24, badge_scale));
    };
    // Scale 1, focus on A: above A's bottom panel, not at the corner of the whole root window.
    const auto on_a = badge_at(WaveOverlayPosition{960, 540}, std::nullopt,
                               wave_overlay_scale(96.0, std::nullopt));
    assert(on_a.x == 1920 - 74 - 24);
    assert(on_a.y == 1032 - 44 - 24);
    // Scale 1, focus on B: left of B's dock, at negative root coordinates.
    const auto on_b = badge_at(WaveOverlayPosition{-1280, 700}, std::nullopt,
                               wave_overlay_scale(96.0, std::nullopt));
    assert(on_b.x == -2560 + 2496 - 74 - 24);
    assert(on_b.y == 1440 - 44 - 24);
    // Scale 2 (Xft.dpi 192): the badge and its margin double. With no focused window the pointer picks the monitor.
    const auto hidpi_a = badge_at(std::nullopt, WaveOverlayPosition{100, 100},
                                  wave_overlay_scale(192.0, std::nullopt));
    assert(hidpi_a.x == 1920 - 148 - 48);
    assert(hidpi_a.y == 1032 - 88 - 48);
    const auto hidpi_b =
        badge_at(std::nullopt, WaveOverlayPosition{-1, 1439},
                 wave_overlay_scale(192.0, std::nullopt));
    assert(hidpi_b.x == -64 - 148 - 48);
    assert(hidpi_b.y == 1440 - 88 - 48);
    // A work area too small for the badge and its margin pins it to the area's origin rather than onto the neighbouring monitor.
    const auto cramped =
        wave_overlay_bottom_right({-300, 50, 150, 100}, 148, 88, 48);
    assert(cramped.x == -300);
    assert(cramped.y == 50);
  }
}
