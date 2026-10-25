#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#include <cstdio>
#include <cstring>
#include <unistd.h>

// 这个入口只用系统事件和原生编辑框，不导入控制器，也不直接调用宿主库。
// 必须在已安装本次构建、已手动选择相应韩语布局的 Mac 登录会话中执行。
static void Pump(NSTimeInterval seconds) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while (end.timeIntervalSinceNow > 0) {
        NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
            untilDate:[NSDate dateWithTimeIntervalSinceNow:0.01] inMode:NSDefaultRunLoopMode dequeue:YES];
        if (event) [NSApp sendEvent:event];
    }
}

static NSString *SelectedSource() {
    TISInputSourceRef source = TISCopyCurrentKeyboardInputSource();
    if (!source) return @"";
    NSString *identifier = [(__bridge NSString *)(CFStringRef)TISGetInputSourceProperty(source, kTISPropertyInputSourceID) copy];
    CFRelease(source);
    return identifier ?: @"";
}

static BOOL AppKitEvents = NO;

// 可选的 AppKit 队列只用于对照诊断，不能据此假定事件经过系统 IMK 路径。
// 正式验收使用下方 CGEventPostToPid，只向验收应用自身投递系统事件。
static NSString *Characters(CGKeyCode code) {
    NSDictionary *keys = @{@0:@"a", @11:@"b", @2:@"d", @5:@"g", @4:@"h", @40:@"k",
        @45:@"n", @12:@"q", @15:@"r", @1:@"s", @32:@"u", @9:@"v", @13:@"w", @7:@"x", @6:@"z",
        @36:@"\r", @51:@"\x7f", @53:@"\x1b", @43:@",", @18:@"1"};
    NSString *value = keys[@(code)];
    if (!value) [NSException raise:@"AcceptanceFailure" format:@"未知按键代码 %u", code];
    return value;
}

static void Key(CGKeyCode code, CGEventFlags flags = 0) {
    for (NSNumber *down in @[@YES, @NO]) {
        if (AppKitEvents) {
            NSString *base = Characters(code);
            NSString *text = (flags & kCGEventFlagMaskShift) ? base.uppercaseString : base;
            NSEvent *event = [NSEvent keyEventWithType:down.boolValue ? NSEventTypeKeyDown : NSEventTypeKeyUp
                location:NSZeroPoint modifierFlags:(NSEventModifierFlags)flags
                timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:NSApp.keyWindow.windowNumber
                context:nil characters:text charactersIgnoringModifiers:base isARepeat:NO keyCode:code];
            [NSApp postEvent:event atStart:NO];
            Pump(0.08);
            continue;
        }
        CGEventRef event = CGEventCreateKeyboardEvent(nullptr, code, down.boolValue);
        if (!event) [NSException raise:@"AcceptanceFailure" format:@"无法建立键盘事件"];
        CGEventSetFlags(event, flags);
        CGEventPostToPid(getpid(), event);
        CFRelease(event);
        Pump(0.08);
    }
}

static void Type(NSString *text) {
    NSDictionary *codes = @{@"a":@0, @"b":@11, @"d":@2, @"g":@5, @"h":@4, @"k":@40,
        @"n":@45, @"q":@12, @"r":@15, @"s":@1, @"u":@32, @"v":@9, @"w":@13, @"x":@7, @"z":@6};
    for (NSUInteger i = 0; i < text.length; ++i) {
        NSString *letter = [text substringWithRange:NSMakeRange(i, 1)];
        NSNumber *code = codes[letter.lowercaseString];
        if (!code) [NSException raise:@"AcceptanceFailure" format:@"未知测试按键 %@", letter];
        Key(code.unsignedShortValue, [letter isEqual:letter.uppercaseString] ? kCGEventFlagMaskShift : 0);
    }
}

static void Expect(NSMutableArray *results, NSTextView *view, NSString *text, BOOL marked, NSString *name) {
    Pump(0.15);
    // NSTextView 的底层字符串会继续变化，证据与断言必须使用同一份不可变快照。
    NSString *actual = [view.string copy];
    const BOOL actualMarked = view.hasMarkedText;
    NSDictionary *result = @{@"case":name, @"expected":text, @"actual":actual,
        @"marked":@(actualMarked), @"expected_marked":@(marked), @"input_source":SelectedSource(),
        @"passed":@([actual isEqual:text] && actualMarked == marked)};
    [results addObject:result];
    if (![result[@"passed"] boolValue]) [NSException raise:@"AcceptanceFailure" format:@"%@：%@", name, result];
}

static void Clear(NSTextView *view) {
    if (view.hasMarkedText) [NSException raise:@"AcceptanceFailure" format:@"清理前组字尚未结束"];
    view.string = @"";
    [view setSelectedRange:NSMakeRange(0, 0)];
}

static NSDictionary *Rectangle(CGRect rect) {
    return @{@"x":@(rect.origin.x), @"y":@(rect.origin.y),
        @"width":@(rect.size.width), @"height":@(rect.size.height)};
}

// 只读取测试输入法的窗口几何，不抓取其他应用画面，也不依赖屏幕录制权限。
static void ExpectCandidateWindow(NSMutableArray *results, NSTextView *view, pid_t serverPID) {
    Pump(0.3);
    NSRect rect = [view firstRectForCharacterRange:view.selectedRange actualRange:nullptr];
    const CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
    const CGRect caret = CGRectMake(rect.origin.x, top - NSMaxY(rect), rect.size.width, rect.size.height);
    NSArray<NSDictionary *> *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    NSUInteger editorIndex = NSNotFound;
    for (NSUInteger i = 0; i < windows.count; ++i) {
        if ([windows[i][(id)kCGWindowOwnerPID] intValue] == getpid() &&
            [windows[i][(id)kCGWindowNumber] integerValue] == view.window.windowNumber) {
            editorIndex = i;
            break;
        }
    }
    NSMutableArray *owned = [NSMutableArray array];
    BOOL passed = NO;
    for (NSUInteger i = 0; i < windows.count; ++i) {
        NSDictionary *window = windows[i];
        if ([window[(id)kCGWindowOwnerPID] intValue] != serverPID) continue;
        CGRect bounds = CGRectZero;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &bounds)) continue;
        const BOOL near = CGRectIntersectsRect(CGRectInset(caret, -48, -48), bounds);
        const BOOL above = editorIndex != NSNotFound && i < editorIndex;
        [owned addObject:@{@"bounds":Rectangle(bounds), @"near_caret":@(near), @"above_editor":@(above)}];
        if (bounds.size.width > 30 && bounds.size.height > 30 &&
            [window[(id)kCGWindowAlpha] doubleValue] > 0 && near && above) passed = YES;
    }
    NSDictionary *result = @{@"case":@"候选窗口靠近光标且位于编辑器前方", @"caret":Rectangle(caret),
        @"input_method_windows":owned, @"passed":@(passed)};
    [results addObject:result];
    if (!passed) [NSException raise:@"AcceptanceFailure" format:@"候选窗口位置：%@", result];
}

// 仅在一次性的 CI 登录会话里模拟用户添加韩语输入源；生产安装只默认启用主模式。
static int EnableCISource(NSString *bundlePath) {
    if (![NSProcessInfo.processInfo.environment[@"GITHUB_ACTIONS"] isEqual:@"true"] || !bundlePath.isAbsolutePath)
        return 2;
    NSBundle *bundle = [NSBundle bundleWithPath:bundlePath];
    if (!bundle.bundleIdentifier) return 2;
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    [NSApp finishLaunching];
    NSString *koreanID = [bundle.bundleIdentifier stringByAppendingString:@".Korean"];
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID:bundle.bundleIdentifier};
    NSMutableArray *inventory = [NSMutableArray array];
    NSMutableArray *attempts = [NSMutableArray array];
    BOOL enabledParent = NO, enabledKorean = NO, enabledPrimary = NO;
    // Mavericks 起模式按独立输入源管理，父记录不能启用时仍须尝试实际韩语模式。
    // 每次重取清单，API 返回 noErr 不代表状态已经生效。
    for (NSString *wanted in @[bundle.bundleIdentifier, koreanID]) {
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5.0];
        BOOL enabled = NO;
        OSStatus status = fnfErr;
        do {
            CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)filter, true);
            if (!sources) break;
            for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
                TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
                NSString *identifier = (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
                if (![identifier isEqual:wanted]) continue;
                CFBooleanRef value = (CFBooleanRef)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
                enabled = value && CFBooleanGetValue(value);
                status = enabled ? noErr : TISEnableInputSource(source);
                break;
            }
            CFRelease(sources);
            if (enabled || status != noErr) break;
            Pump(0.1);
        } while (deadline.timeIntervalSinceNow > 0);
        [attempts addObject:@{@"id":wanted, @"status":@(status), @"enabled":@(enabled)}];
    }
    CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)filter, true);
    if (sources) {
        for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
            TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
            id identifier = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
            id enabled = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
            id type = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceType);
            id selectable = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsSelectCapable);
            [inventory addObject:@{@"id":identifier ?: @"", @"enabled":enabled ?: @NO,
                @"type":type ?: @"", @"select_capable":selectable ?: @NO}];
            if ([identifier isEqual:bundle.bundleIdentifier]) enabledParent = [enabled boolValue];
            if ([identifier isEqual:koreanID]) enabledKorean = [enabled boolValue];
            if ([identifier isEqual:[bundle.bundleIdentifier stringByAppendingString:@".Hans"]]) enabledPrimary = [enabled boolValue];
        }
        CFRelease(sources);
    }
    NSDictionary *report = @{@"bundle_id":bundle.bundleIdentifier, @"sources":inventory,
        @"enable_attempts":attempts, @"parent_enabled":@(enabledParent),
        @"korean_mode_enabled":@(enabledKorean), @"primary_mode_enabled":@(enabledPrimary),
        @"client_bundle_id":NSBundle.mainBundle.bundleIdentifier ?: @"",
        @"system_input_executed":@NO,
        @"passed":@(enabledKorean)};
    NSData *data = [NSJSONSerialization dataWithJSONObject:report options:NSJSONWritingPrettyPrinted error:nil];
    puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
    return enabledKorean ? 0 : 1;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc == 3 && strcmp(argv[1], "--enable-ci-source") == 0)
            return EnableCISource(@(argv[2]));
        if (argc == 2 && strcmp(argv[1], "--probe") == 0) {
            NSDictionary *probe = @{@"system_input_executed":@NO,
                @"event_posting_allowed":@(CGPreflightPostEventAccess()), @"input_source":SelectedSource()};
            NSData *data = [NSJSONSerialization dataWithJSONObject:probe options:NSJSONWritingPrettyPrinted error:nil];
            puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
            return 0;
        }
        if ((argc != 4 && argc != 5) || (argc == 5 && strcmp(argv[4], "--appkit-events") != 0) ||
            (strcmp(argv[1], "romaja") != 0 && strcmp(argv[1], "dubeolsik") != 0)) {
            fputs("用法：korean-system-input-acceptance <romaja|dubeolsik> <已安装输入法.app绝对路径> <报告.json绝对路径> [--appkit-events]\n", stderr);
            return 2;
        }
        AppKitEvents = argc == 5;
        NSString *layout = @(argv[1]), *bundlePath = @(argv[2]), *output = @(argv[3]);
        if (!bundlePath.isAbsolutePath || !output.isAbsolutePath) return 2;
        NSBundle *bundle = [NSBundle bundleWithPath:bundlePath];
        NSString *sourceID = [bundle.bundleIdentifier stringByAppendingString:@".Korean"];
        NSMutableArray *results = [NSMutableArray array];
        NSMutableDictionary *report = [@{@"layout":layout, @"bundle_path":bundlePath, @"cases":results,
            @"event_transport":AppKitEvents ? @"appkit_event_queue" : @"cg_event_post",
            @"requested_input_source":sourceID,
            @"bundle_revision":bundle.infoDictionary[@"MSIMEInputSourceRevision"] ?: @"",
            @"client_bundle_id":NSBundle.mainBundle.bundleIdentifier ?: @"",
            @"system_input_executed":@NO, @"passed":@NO} mutableCopy];
        TISInputSourceRef previous = nullptr;
        NSWindow *window = nil;
        int status = 1;
        @try {
            [NSApplication sharedApplication];
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
            [NSApp finishLaunching];
            if (!AppKitEvents && !CGPreflightPostEventAccess())
                [NSException raise:@"AcceptancePrerequisite" format:@"当前进程没有系统按键投递权限；未请求或修改权限，未执行验收"];
            NSURL *registered = bundle.bundleIdentifier
                ? [NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:bundle.bundleIdentifier] : nil;
            if (!registered || ![registered.URLByResolvingSymlinksInPath.path isEqual:[bundle.bundleURL URLByResolvingSymlinksInPath].path])
                [NSException raise:@"AcceptancePrerequisite" format:@"LaunchServices 登记的输入法与指定构建不一致"];
            NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID:bundle.bundleIdentifier};
            // 命令行进程使用 includeAllInstalled，再明确核对启用状态；false 会按当前语言筛掉其他模式。
            CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)filter, true);
            CFIndex target = -1;
            NSMutableArray *inventory = [NSMutableArray array];
            if (sources) for (CFIndex index = 0; index < CFArrayGetCount(sources); ++index) {
                TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, index);
                id identifier = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
                id enabled = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
                id selectable = (__bridge id)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsSelectCapable);
                [inventory addObject:@{@"id":identifier ?: @"", @"enabled":enabled ?: @NO,
                    @"select_capable":selectable ?: @NO}];
                if ([identifier isEqual:sourceID]) target = index;
            }
            report[@"installed_input_sources"] = inventory;
            if (target < 0) {
                if (sources) CFRelease(sources);
                [NSException raise:@"AcceptancePrerequisite" format:@"未找到本次构建的输入源 %@", sourceID];
            }
            CFBooleanRef enabled = (CFBooleanRef)TISGetInputSourceProperty(
                (TISInputSourceRef)CFArrayGetValueAtIndex(sources, target), kTISPropertyInputSourceIsEnabled);
            if (!enabled || !CFBooleanGetValue(enabled)) {
                CFRelease(sources);
                [NSException raise:@"AcceptancePrerequisite" format:@"已安装的韩语输入源尚未启用"];
            }
            previous = TISCopyCurrentKeyboardInputSource();
            window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100, 100, 700, 400)
                styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
            window.releasedWhenClosed = NO;
            window.title = @"水杉韩语系统输入验收";
            NSTextView *first = [[NSTextView alloc] initWithFrame:NSMakeRect(10, 210, 680, 180)];
            NSTextView *second = [[NSTextView alloc] initWithFrame:NSMakeRect(10, 10, 680, 180)];
            [window.contentView addSubview:first];
            [window.contentView addSubview:second];
            [window makeKeyAndOrderFront:nil];
            [NSApp activateIgnoringOtherApps:YES];
            [window makeFirstResponder:first];
            Pump(1);
            // 先建立活动的文本输入上下文，避免无文档的命令行进程在选择输入法时返回 paramErr。
            [first.inputContext activate];
            report[@"application_active"] = @(NSApp.isActive);
            report[@"frontmost_pid"] = @(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier);
            if (previous) report[@"baseline_selection_status"] = @(TISSelectInputSource(previous));
            NSMutableArray *available = [NSMutableArray array];
            for (NSString *identifier in first.inputContext.keyboardInputSources)
                if ([identifier hasPrefix:bundle.bundleIdentifier]) [available addObject:identifier];
            report[@"input_context_available_sources"] = available;
            OSStatus selected = TISSelectInputSource((TISInputSourceRef)CFArrayGetValueAtIndex(sources, target));
            CFRelease(sources);
            report[@"tis_select_status"] = @(selected);
            report[@"selection_transport"] = @"TISSelectInputSource";
            if (selected != noErr) {
                first.inputContext.selectedKeyboardInputSource = sourceID;
                report[@"selection_transport"] = @"NSTextInputContext.selectedKeyboardInputSource";
            }
            // 冷启动可异步恢复模式；只接受本次验收请求的韩语输入源。
            BOOL (^sourceReady)(void) = ^BOOL {
                NSString *actual = SelectedSource();
                return [actual isEqual:sourceID];
            };
            NSDate *sourceDeadline = [NSDate dateWithTimeIntervalSinceNow:5.0];
            while (!sourceReady() && sourceDeadline.timeIntervalSinceNow > 0) Pump(0.05);
            report[@"selected_input_source"] = SelectedSource();
            report[@"input_context_source"] = first.inputContext.selectedKeyboardInputSource ?: @"";
            if (!sourceReady())
                [NSException raise:@"AcceptancePrerequisite" format:@"活动文本框无法选择本次输入源（TIS 状态 %d）；实际为 %@", (int)selected, SelectedSource()];
            NSArray<NSRunningApplication *> *servers = [NSRunningApplication runningApplicationsWithBundleIdentifier:bundle.bundleIdentifier];
            NSRunningApplication *server = nil;
            for (NSRunningApplication *application in servers) {
                if (application.processIdentifier != getpid() &&
                    [application.bundleURL.URLByResolvingSymlinksInPath.path isEqual:bundle.bundleURL.URLByResolvingSymlinksInPath.path]) {
                    server = application;
                    break;
                }
            }
            if (!server) [NSException raise:@"AcceptancePrerequisite" format:@"本次已安装输入法没有独立的运行进程"];
            report[@"input_method_pid"] = @(server.processIdentifier);
            report[@"client_pid"] = @(getpid());
            report[@"system_input_executed"] = @YES;
            BOOL romaja = [layout isEqual:@"romaja"];
            Type(romaja ? @"hang" : @"gks");
            Expect(results, first, romaja ? @"항" : @"한", YES, @"预编辑");
            Type(romaja ? @"ug" : @"rnr");
            Expect(results, first, @"한국", YES, @"后续组字不重复前缀");
            Key(36);
            Expect(results, first, @"한국\n", NO, @"Return 提交并交给应用换行");
            Clear(first);
            if (romaja) {
                Type(@"bang"); Key(51);
                Expect(results, first, @"반", YES, @"按原始拼写退格");
                Key(53); Expect(results, first, @"", NO, @"Escape 取消");
                Type(@"gwa"); Key(51);
                Expect(results, first, @"구", YES, @"复合元音退格"); Key(53);
                Type(@"xa"); Expect(results, first, @"아", YES, @"x 声母"); Key(53);
                Type(@"Ga"); Key(43);
                Expect(results, first, @"까,", NO, @"紧音和半角标点"); Clear(first);
                Type(@"hanqvz");
                Expect(results, first, @"한qvz", NO, @"布局外字母保留"); Clear(first);
            }
            Type(romaja ? @"han" : @"gks"); Key(36, kCGEventFlagMaskAlternate);
            ExpectCandidateWindow(results, first, server.processIdentifier);
            Key(18);
            Expect(results, first, @"韓", NO, @"系统候选窗口选择汉字"); Clear(first);
            Type(romaja ? @"han" : @"gks");
            [window makeFirstResponder:second]; Pump(0.5);
            Expect(results, first, @"한", NO, @"失焦文字留在原输入框");
            Type(romaja ? @"hangug" : @"gksrnr");
            Expect(results, second, @"한국", YES, @"新输入框独立组字"); Key(36);
            Expect(results, second, @"한국\n", NO, @"新输入框提交");
            Expect(results, first, @"한", NO, @"原输入框不被改写");
            NSBitmapImageRep *bitmap = [window.contentView bitmapImageRepForCachingDisplayInRect:window.contentView.bounds];
            [window.contentView cacheDisplayInRect:window.contentView.bounds toBitmapImageRep:bitmap];
            NSString *imagePath = [[output stringByDeletingPathExtension] stringByAppendingPathExtension:@"png"];
            NSData *image = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
            if (![image writeToFile:imagePath atomically:YES])
                [NSException raise:@"AcceptanceFailure" format:@"无法保存原生编辑框截图"];
            report[@"editor_image"] = imagePath;
            report[@"passed"] = @YES;
            status = 0;
        } @catch (NSException *exception) {
            report[@"error"] = exception.reason ?: exception.name;
            if ([exception.name isEqual:@"AcceptancePrerequisite"]) status = 77;
        } @finally {
            [window close];
            if (previous) { TISSelectInputSource(previous); CFRelease(previous); }
            NSData *data = [NSJSONSerialization dataWithJSONObject:report options:NSJSONWritingPrettyPrinted error:nil];
            if (![data writeToFile:output atomically:YES]) status = 2;
            puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
        }
        return status;
    }
}
