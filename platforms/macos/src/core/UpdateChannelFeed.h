#pragma once

#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// 更新通道与 Sparkle feed 地址的对应规则，输入法（UpdateController.mm）和完整设置应用（crates/host-macos/native/updater.mm）共用这一份。通道的取值与偏好 update_channel 相同（client-core 的 UpdateChannel）：stable、beta、nightly。
//
// Info.plist 里的 SUFeedURL 始终是稳定版的 feed：输入法是 <base>appcast-<版本 id>.xml，设置应用是 <base>appcast-settings-<版本 id>.xml，打包与各版本改写（edition_bundle.py、settings_updater.py）都不因通道而变。测试版和每日构建的 feed 在同一目录下、版本 id 前面插入通道名：appcast-beta-full.xml、appcast-settings-nightly-wubi.xml。platforms/macos/scripts/sparkle_feed.py 的 feed_name 是打包和发布一侧的同一条规则。

// 偏好里的 update_channel 读成通道：缺省（稳定版不写进文档）、不认识的字符串和类型不对的值都按稳定版，与 client-core 读偏好文档的规则一致。
static inline NSString *MSIMEUpdateChannelFromPreference(id _Nullable value)
{
    if ([value isKindOfClass:NSString.class] && ([value isEqualToString:@"beta"] || [value isEqualToString:@"nightly"]))
        return [value copy];
    return @"stable";
}

// 宿主请求里的通道严格解析：只认 stable、beta、nightly，其他值返回 nil，由调用方当作请求不合法。
static inline NSString *_Nullable MSIMEUpdateChannelParse(NSString *_Nullable value)
{
    if ([value isEqualToString:@"stable"] || [value isEqualToString:@"beta"] || [value isEqualToString:@"nightly"])
        return [value copy];
    return nil;
}

// 更新包属于哪个通道，从文件名读：测试版和每日构建的更新包名是 <前缀>-<版本>-<通道>-<构建号>-input-method.zip（设置应用是 -settings.zip），其余都算稳定版。与 sparkle_feed.py 的 archive_channel 是同一条规则。复制到更宽通道的 appcast 指向的仍是原通道的更新包，所以每日构建的 feed 里也可能是一个稳定版的更新包。
static inline NSString *MSIMEUpdateChannelOfArchiveName(NSString *_Nullable name)
{
    if (![name isKindOfClass:NSString.class]) return @"stable";
    NSRegularExpression *pattern = [NSRegularExpression
        regularExpressionWithPattern:@"-(beta|nightly)-[1-9][0-9]*-(?:input-method|settings)\\.zip\\z"
                             options:0
                               error:nil];
    NSTextCheckingResult *match = [pattern firstMatchInString:name options:0 range:NSMakeRange(0, name.length)];
    return match ? [name substringWithRange:[match rangeAtIndex:1]] : @"stable";
}

// 选了 channel 的用户是否接受 archiveChannel 通道的构建：更宽的通道包含更窄的，每日构建接受三者，测试版接受测试版和正式版，正式版只接受正式版。与 sparkle_feed.py 的 WIDER_CHANNELS 一致。两个参数都按 MSIMEUpdateChannelFromPreference 读，nil 和不认识的值算稳定版。
static inline BOOL MSIMEUpdateChannelAccepts(NSString *_Nullable channel, NSString *_Nullable archiveChannel)
{
    NSArray<NSString *> *order = @[ @"stable", @"beta", @"nightly" ];
    NSUInteger chosen = [order indexOfObject:MSIMEUpdateChannelFromPreference(channel)];
    NSUInteger offered = [order indexOfObject:MSIMEUpdateChannelFromPreference(archiveChannel)];
    return offered <= chosen;
}

// 从稳定版 feed 推出某个通道的 feed。稳定版原样返回；地址不是 https、带查询或片段、文件名不是 appcast-[settings-]<版本 id>.xml 时返回 nil，调用方这时退回 Info.plist 的 SUFeedURL（即稳定版），不会去请求一个拼错的地址。
static inline NSString *_Nullable MSIMEUpdateFeedURLForChannel(NSString *_Nullable stableFeed, NSString *channel)
{
    if (![stableFeed isKindOfClass:NSString.class] || stableFeed.length == 0) return nil;
    NSString *parsed = MSIMEUpdateChannelParse(channel);
    if (!parsed) return nil;
    NSURLComponents *components = [NSURLComponents componentsWithString:stableFeed];
    if (!components || ![components.scheme isEqualToString:@"https"] || components.host.length == 0 ||
        components.query != nil || components.fragment != nil)
        return nil;
    NSString *path = components.path;
    NSString *name = path.lastPathComponent;
    NSString *prefix = [name hasPrefix:@"appcast-settings-"] ? @"appcast-settings-"
                       : [name hasPrefix:@"appcast-"]        ? @"appcast-"
                                                             : nil;
    if (!prefix || ![name hasSuffix:@".xml"] || name.length <= prefix.length + 4) return nil;
    NSString *edition = [name substringWithRange:NSMakeRange(prefix.length, name.length - prefix.length - 4)];
    // 已经是某个通道的 feed（文件名里版本 id 之前带着通道名）就不是稳定版的 feed，不再往上叠。
    if ([edition hasPrefix:@"beta-"] || [edition hasPrefix:@"nightly-"] || [edition containsString:@"/"]) return nil;
    if ([parsed isEqualToString:@"stable"]) return stableFeed;
    NSString *channelName = [NSString stringWithFormat:@"%@%@-%@.xml", prefix, parsed, edition];
    components.path = [[path substringToIndex:path.length - name.length] stringByAppendingString:channelName];
    return components.string;
}

NS_ASSUME_NONNULL_END
