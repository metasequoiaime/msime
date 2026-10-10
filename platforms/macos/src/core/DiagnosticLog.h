#pragma once

#include <string>
#include <string_view>

// macOS host diagnostics are deliberately limited to state/event labels, counts, geometry, timings and error categories. They never receive keystrokes, input text, candidates, credentials, or provider responses.
void msime_macos_diagnostic_configure(const std::string &directory,
                                      bool enabled) noexcept;
void msime_macos_diagnostic_write(std::string_view event) noexcept;

// One relaxed load, so hot paths such as key handling and candidate layout can skip timing and formatting entirely while the log is off, like MSIME-Windows' CandidateDiagLog::IsEnabled().
bool msime_macos_diagnostic_enabled() noexcept;

// printf-style write for call sites that format numbers. Returns at once while the log is off; the formatted event is cut to the same per-event cap as msime_macos_diagnostic_write.
void msime_macos_diagnostic_writef(const char *format, ...) noexcept
    __attribute__((format(printf, 1, 2)));

// The sink registered with msime_client_set_diagnostic_sink; writes "host_api: <category>", the fixed text before the line's first colon, and drops the rest: the pack id, the file and the error text that Host API puts after it. Like every other event it is dropped while the log is off and nothing is kept for later, so a failure Host API reports once - such as a helpcode pack fallback while the session is created, before the first preference load has configured this log - reaches the log only when it is reported again with the log on. Any thread; takes only this log's own lock, never throws.
void msime_macos_diagnostic_host_line(const char *line) noexcept;
