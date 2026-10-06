#import <Foundation/Foundation.h>

// Anonymous usage reporting for the input method process, through client-core's telemetry queue (msime_client_telemetry_* in msime_client.h). One input method process lifetime is one session; usage_reporting in the shared preferences (default on) decides whether anything is recorded or sent.

// Starts the session, installs the crash handlers and sends what is queued from a background queue. Call once, after the runtime options are current and before IMK sessions are accepted. preferencesDirectory is the shared preferences directory from the runtime options, or nil when the host is not configured yet (reporting then follows the default, on).
void MSIMEUsageReportingStart(NSString *_Nullable preferencesDirectory);

// The process is exiting normally: queues the session event. Safe to call more than once.
void MSIMEUsageReportingStop(void);

// The directory holding the queue, the install id, the session marker and crash records.
NSString *_Nonnull MSIMEUsageReportingDirectory(void);
