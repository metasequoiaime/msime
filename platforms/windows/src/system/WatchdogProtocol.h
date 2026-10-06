#pragma once

#include <cstdint>

// The Server and Watchdog are separate processes, so keep their lifecycle contract in one header.
namespace msime::windows::watchdog_protocol {
inline constexpr std::uint32_t stop_exit_code = 0x4D530001u;
inline constexpr std::uint32_t restart_exit_code = 0x4D530002u;
inline constexpr wchar_t managed_argument[] = L"--watchdog-managed";
} // namespace msime::windows::watchdog_protocol
