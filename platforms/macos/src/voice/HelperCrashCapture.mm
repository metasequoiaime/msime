#include "../../../../shared/voice/HelperCrashCapture.h"
#include "../../../common/Telemetry.h"
#import "../settings/RuntimeOptions.h"

#include <mach-o/dyld.h>
#include <vector>

namespace msime::voice {
namespace {
// helper 不是 bundle 主程序；从真实可执行文件定位所在输入法，不能按 mainBundle 的回退身份认领 full 同意。
NSBundle *owning_bundle() {
  uint32_t size = 0;
  _NSGetExecutablePath(nullptr, &size);
  std::vector<char> path(size);
  if (_NSGetExecutablePath(path.data(), &size) != 0)
    return nil;
  NSURL *executable = [[NSURL fileURLWithFileSystemRepresentation:path.data()
                                                  isDirectory:NO relativeToURL:nil] URLByResolvingSymlinksInPath];
  NSURL *macos = executable.URLByDeletingLastPathComponent;
  NSURL *contents = macos.URLByDeletingLastPathComponent;
  NSURL *bundle = contents.URLByDeletingLastPathComponent;
  if (![macos.lastPathComponent isEqualToString:@"MacOS"] ||
      ![contents.lastPathComponent isEqualToString:@"Contents"] ||
      ![bundle.pathExtension isEqualToString:@"app"])
    return nil;
  return [NSBundle bundleWithURL:bundle];
}

std::optional<telemetry::Host> read_host() {
  @autoreleasepool {
    NSBundle *bundle = owning_bundle();
    NSDictionary *info = bundle.infoDictionary;
    if (!info)
      return std::nullopt;
    NSString *edition = MSIMEEditionIdentifierIn(info);
    NSCharacterSet *invalid = [[NSCharacterSet characterSetWithCharactersInString:@"abcdefghijklmnopqrstuvwxyz0123456789-"] invertedSet];
    if ([edition rangeOfCharacterFromSet:invalid].location != NSNotFound)
      return std::nullopt;
    const BOOL full = [edition isEqualToString:MSIMEFullEditionIdentifier];
    const NSRange separator = [MSIMEFullInputMethodBundleIdentifier rangeOfString:@"." options:NSBackwardsSearch];
    NSString *inputPrefix = [MSIMEFullInputMethodBundleIdentifier substringToIndex:NSMaxRange(separator)];
    NSString *identifier = full ? MSIMEFullInputMethodBundleIdentifier
        : [inputPrefix stringByAppendingString:edition];
    NSString *settings = full ? MSIMEFullSettingsBundleIdentifier
        : [MSIMEFullSettingsBundleIdentifier stringByAppendingFormat:@".%@", edition];
    if (![MSIMEEditionStringIn(info, @"CFBundleIdentifier") isEqualToString:identifier] ||
        (!full && ![MSIMEEditionStringIn(info, @"MSIMESettingsBundleIdentifier") isEqualToString:settings]))
      return std::nullopt;
    NSString *version = MSIMEEditionStringIn(info, @"CFBundleShortVersionString");
    NSURL *support = [[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory
                                                          inDomains:NSUserDomainMask] firstObject];
    if (!version || !support)
      return std::nullopt;
    NSString *options = NSProcessInfo.processInfo.environment[@"MSIME_CLIENT_HOST_OPTIONS"];
    if (!options)
      options = [bundle pathForResource:@"runtime-options" ofType:@"json"] ?:
          [[[support URLByAppendingPathComponent:settings isDirectory:YES]
              URLByAppendingPathComponent:@"runtime-options.json"] path];
    if (!options.isAbsolutePath)
      return std::nullopt;
    NSData *data = MSIMEReadRuntimeOptionsData(options);
    id document = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    id preferences = [document isKindOfClass:NSDictionary.class] ? document[@"preferences_directory"] : nil;
    if (![preferences isKindOfClass:NSString.class] || ![preferences isAbsolutePath])
      return std::nullopt;
    NSURL *directory = [support URLByAppendingPathComponent:MSIMEUsageReportingDirectoryNameIn(info) isDirectory:YES];
    return telemetry::Host{"macos", version.UTF8String, directory.fileSystemRepresentation,
                           std::nullopt, [preferences fileSystemRepresentation], "voice-local", true,
                           edition.UTF8String};
  }
}
} // namespace

struct HelperCrashCapture::State {
  telemetry::CaptureOnly capture{read_host};
};

HelperCrashCapture::HelperCrashCapture() noexcept {
  try {
    state_ = std::make_unique<State>();
  } catch (...) {
  }
}

HelperCrashCapture::~HelperCrashCapture() = default;

} // namespace msime::voice
