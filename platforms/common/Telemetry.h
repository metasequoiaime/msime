#pragma once
#include <filesystem>
#include <optional>
#include <string>

// Anonymous usage reporting for the native Windows and Linux hosts: a thin wrapper over the Host API's msime_client_telemetry_* functions, which own the queue, the install id, the daily active event, sessions and delivery to https://api.msime.app/v1/telemetry/events. Nothing here sends anything itself, and nothing on a crash path touches the network: crash handlers only write the session's crash record, which the next start turns into crash and session_crash events.
namespace msime::telemetry {
struct Host {
  // windows or linux.
  std::string platform;
  // The real app version.
  std::string version;
  // Where the telemetry files live (absolute); default_directory() for the IBus host and the Windows Server.
  std::filesystem::path directory;
  // The user's usage_reporting switch when the host has already read it. Unset means it is read from preferences_directory/preferences.json, and with neither the default (on) applies.
  std::optional<bool> enabled;
  std::filesystem::path preferences_directory;
};

// %LOCALAPPDATA%\MSIME on Windows; $XDG_STATE_HOME/msime (or ~/.local/state/msime) elsewhere.
std::filesystem::path default_directory();

// Starts this process's reporting session: closes the previous one, queues its crash records and today's active, and arms the crash handlers with this session's crash record path. With reporting off it clears everything queued instead. File I/O only, no network. Returns whether reporting is on.
bool begin(const Host &host);
// The host is exiting normally: queues the session event (sent on the next start). File I/O only.
void end();
// Sends the queue. Blocks on the network; background threads only.
void flush();
// A detached thread that flushes now and every 30 minutes, so a host that runs for days still reports each day's active and retries what could not be sent. The flush itself reads the switch again, so turning reporting off stops it at the next round.
void start_flushing();
// The user changed the usage_reporting switch while the host runs (hosts that pass Host::enabled): off clears the queue and disarms crash capture; on starts a session as at host start. File I/O only.
void set_enabled(bool enabled);

// For a std::terminate handler (may allocate): writes the crash record of the running session with the current exception as the message and this thread's stack. Does nothing when no session runs.
void record_terminate();
// Installs crash capture that only writes to disk: SetUnhandledExceptionFilter on Windows, sigaction for SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT elsewhere. POSIX handlers chain to whatever was installed before, so a host process with its own crash handler (Fcitx5) keeps it. Call once, from the main thread.
void install_crash_handlers();
// Restores the handlers install_crash_handlers replaced; for a plugin whose code may be unloaded while the process lives on.
void remove_crash_handlers();

// Exposed for tests: the crash summary of the current exception and a symbolic stack of the calling thread, as record_terminate writes them.
std::string current_exception_summary();
std::string current_stack(int skip);
}
