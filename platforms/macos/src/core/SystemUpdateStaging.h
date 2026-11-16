#pragma once

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// 输入法装在 /Library/Input Methods 时怎么更新自己。那个目录归 root，本进程写不了，Sparkle 在这种位置上的静默安装会放弃、改成弹窗要管理员密码。所以 Sparkle 更新的是用户目录里的一份暂存副本（SPUUpdater 的 hostBundle），暂存副本变新之后提交给 root 更新助手（crates/macos-system-updater），由它核对签名和构建号后换进系统目录，本进程再在用户不在打字时退出，让系统拉起新的那份。决定见 .agents/notes/implemented/bug-fix/2026-11-16-macos-system-directory-install-and-updater.md。
//
// 这里只有判断和文件操作，不碰 Sparkle，测试不用启动更新器。

// 与 crates/macos-system-updater 的 Layout::system 一致。
static NSString *const MSIMESystemUpdaterInboxPath = @"/Library/Application Support/app.msime.system-updater/inbox";

// macOS 只把这个目录里的第三方输入法算作安全输入源，也只有这里的 bundle 本进程换不了。
static inline BOOL MSIMEUpdateHostIsSystemInstall(NSString *_Nullable bundlePath)
{
    return [bundlePath.stringByStandardizingPath hasPrefix:@"/Library/Input Methods/"];
}

// 磁盘上那份 bundle 现在的构建号，读不出来时为 0。每次都重新读 Info.plist：NSBundle 会缓存启动时的内容，而这里要看的正是 bundle 被换掉之后的值。
static inline long long MSIMEUpdateBundleBuildOnDisk(NSURL *bundleURL)
{
    NSURL *info = [bundleURL URLByAppendingPathComponent:@"Contents/Info.plist"];
    NSDictionary *plist = [NSDictionary dictionaryWithContentsOfURL:info error:nil];
    id build = plist[@"CFBundleVersion"];
    return [build respondsToSelector:@selector(longLongValue)] ? MAX([build longLongValue], 0) : 0;
}

// 暂存副本放在缓存目录里：Spotlight 和 LaunchServices 不扫那里，不会把它登记成第二份输入法（同一个标识有两条登记时系统会把输入源列两遍）。缓存被清掉时下次启动重新拷一份。
static inline NSURL *MSIMEUpdateStagingURL(NSURL *cachesURL, NSString *bundleIdentifier, NSString *bundleName)
{
    NSURL *directory = [[cachesURL URLByAppendingPathComponent:bundleIdentifier isDirectory:YES] URLByAppendingPathComponent:@"system-update" isDirectory:YES];
    return [directory URLByAppendingPathComponent:bundleName isDirectory:YES];
}

// 让暂存副本至少和已装的那份一样新：没有、读不出构建号或比已装的旧时，用已装的那份重新拷。已经更新的（Sparkle 更新过、还没换进系统目录）原样留着，否则下好的更新就丢了。
static inline BOOL MSIMEUpdatePrepareStaging(NSFileManager *fileManager, NSURL *installedURL, NSURL *stagingURL)
{
    long long installed = MSIMEUpdateBundleBuildOnDisk(installedURL);
    if (installed <= 0) return NO;
    if (MSIMEUpdateBundleBuildOnDisk(stagingURL) >= installed) return YES;
    [fileManager removeItemAtURL:stagingURL error:nil];
    if (![fileManager createDirectoryAtURL:stagingURL.URLByDeletingLastPathComponent withIntermediateDirectories:YES attributes:nil error:nil]) return NO;
    if (![fileManager copyItemAtURL:installedURL toURL:stagingURL error:nil])
    {
        [fileManager removeItemAtURL:stagingURL error:nil];
        return NO;
    }
    return YES;
}

// 把暂存副本提交给更新助手：先用点开头的名字拷进收件目录，拷完再改名。助手不碰点开头的条目，所以它看到的总是一份拷完的 bundle。收件目录不存在（助手没装）时返回 NO。
static inline BOOL MSIMEUpdateSubmitToSystemUpdater(NSFileManager *fileManager, NSURL *stagingURL, NSURL *inboxURL, NSString *name)
{
    NSURL *copying = [inboxURL URLByAppendingPathComponent:[@"." stringByAppendingString:name] isDirectory:YES];
    NSURL *submitted = [inboxURL URLByAppendingPathComponent:name isDirectory:YES];
    [fileManager removeItemAtURL:copying error:nil];
    if (![fileManager copyItemAtURL:stagingURL toURL:copying error:nil] || ![fileManager moveItemAtURL:copying toURL:submitted error:nil])
    {
        [fileManager removeItemAtURL:copying error:nil];
        return NO;
    }
    return YES;
}

typedef NS_ENUM(NSInteger, MSIMESystemUpdateStep) {
    // 没有要做的，或者只能等：让 Sparkle 把它下好的更新装进暂存副本（没有待装的更新时它什么也不做）。
    MSIMESystemUpdateStepInstallIntoStaging,
    // 暂存副本比系统目录里的新，还没为这个构建提交过。
    MSIMESystemUpdateStepSubmit,
    // 系统目录里的那份已经比正在运行的新：本进程退出，系统按需拉起新的。
    MSIMESystemUpdateStepRestart,
};

// running 是本进程启动时的构建号，installed 和 staged 是磁盘上两份现在的构建号，submitted 是上一次提交给助手的构建号（没提交过为 0）。同一个构建只提交一次：助手拒绝它时再交一遍也是同样的结果，每次都要拷两百多 MB。
static inline MSIMESystemUpdateStep MSIMESystemUpdateNextStep(long long running, long long installed, long long staged, long long submitted)
{
    if (installed > running) return MSIMESystemUpdateStepRestart;
    if (staged > installed && staged != submitted) return MSIMESystemUpdateStepSubmit;
    return MSIMESystemUpdateStepInstallIntoStaging;
}

NS_ASSUME_NONNULL_END
