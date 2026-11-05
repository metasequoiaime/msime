#pragma once
#import <AppKit/AppKit.h>
#import <InputMethodKit/InputMethodKit.h>
#import "MSIMEClientSession.h"
#import "GlobalVoicePreferenceNotification.h"

// 范围值来自共享 Rust 偏好；本层只画原生菜单，并通过 Host API 修改自己拥有的一个字段。
static inline BOOL MSIMEVoiceScopeSnapshotValid(NSDictionary *snapshot) {
    if (![snapshot isKindOfClass:NSDictionary.class]) return NO;
    id revision = snapshot[@"revision"], preferences = snapshot[@"preferences"];
    return [revision isKindOfClass:NSNumber.class] &&
        CFGetTypeID((__bridge CFTypeRef)revision) != CFBooleanGetTypeID() &&
        [revision compare:@0] != NSOrderedAscending &&
        [revision compare:@([revision unsignedLongLongValue])] == NSOrderedSame &&
        [preferences isKindOfClass:NSDictionary.class] &&
        [preferences[@"voice_input"] isKindOfClass:NSDictionary.class];
}

static inline NSMenuItem *MSIMEGlobalVoiceScopeMenuItem(NSDictionary *snapshot, id target, SEL action) {
    const BOOL available = MSIMEVoiceScopeSnapshotValid(snapshot);
    const BOOL global = available && [snapshot[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:@YES];
    NSMenuItem *root = [[NSMenuItem alloc] initWithTitle:@"语音快捷键范围" action:nil keyEquivalent:@""];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:root.title];
    menu.autoenablesItems = NO;
    for (NSInteger index = 0; index < 2; ++index) {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:index ? @"任意输入法" : @"仅水杉输入法"
            action:action keyEquivalent:@""];
        item.target = target;
        item.tag = index;
        item.enabled = available;
        item.state = available && global == (index == 1) ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:item];
    }
    root.submenu = menu;
    root.enabled = available;
    if (!available) root.toolTip = @"无法读取语音设置，请重试。";
    return root;
}

static inline BOOL MSIMESaveGlobalVoiceScope(BOOL enabled, NSDictionary *(^load)(void),
    NSDictionary *(^save)(uint64_t, NSDictionary *)) {
    // 每次重试都重读最新文档，只覆盖范围，不能回写旧提供商、快捷键或其他设置。
    for (NSUInteger attempt = 0; attempt < 2; ++attempt) {
        NSDictionary *snapshot = load();
        if (!MSIMEVoiceScopeSnapshotValid(snapshot)) return NO;
        NSMutableDictionary *preferences = [snapshot[@"preferences"] mutableCopy];
        NSMutableDictionary *voice = [preferences[@"voice_input"] mutableCopy];
        voice[@"hotkey_global"] = @(enabled);
        preferences[@"voice_input"] = voice;
        NSMutableDictionary *updated = [snapshot mutableCopy];
        updated[@"preferences"] = preferences;
        if (save([snapshot[@"revision"] unsignedLongLongValue], updated)) return YES;
    }
    return NO;
}

static inline NSDictionary *MSIMELoadGlobalVoiceScope(NSString *directory) {
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath) return nil;
    return [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
}

static inline BOOL MSIMEStoreGlobalVoiceScope(NSString *directory, BOOL enabled) {
    return MSIMESaveGlobalVoiceScope(enabled, ^{ return MSIMELoadGlobalVoiceScope(directory); },
        ^NSDictionary *(uint64_t revision, NSDictionary *snapshot) {
            return [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:revision snapshot:snapshot error:nil];
        });
}

typedef NSDictionary *(^MSIMEVoiceScopeLoad)(NSString *);
typedef NSDictionary *(^MSIMEVoiceScopeSave)(NSString *, uint64_t, NSDictionary *);

// 同进程的菜单读写按点击顺序串行执行，文件锁和 fsync 都不能阻塞输入法主线程。
inline dispatch_queue_t MSIMEGlobalVoiceScopeQueue(void) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ queue = dispatch_queue_create("app.msime.voice-scope", DISPATCH_QUEUE_SERIAL); });
    return queue;
}

// 状态只在主线程访问。原始菜单可能被 IMK 复制后释放，因此快照不能只存在菜单对象里。
inline NSMutableDictionary *MSIMEGlobalVoiceScopeStates(void) {
    static NSMutableDictionary *states;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ states = [NSMutableDictionary dictionary]; });
    return states;
}

static inline NSMutableDictionary *MSIMEVoiceScopeState(NSString *directory, BOOL create) {
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath) return nil;
    NSMutableDictionary *state = MSIMEGlobalVoiceScopeStates()[directory];
    if (!state && create) {
        state = [@{@"roots":NSHashTable.weakObjectsHashTable} mutableCopy];
        MSIMEGlobalVoiceScopeStates()[directory] = state;
    }
    return state;
}

static inline void MSIMEUpdateVoiceScopeMenus(NSMutableDictionary *state) {
    NSDictionary *snapshot = state[@"snapshot"];
    const BOOL available = ![state[@"writing"] boolValue] && MSIMEVoiceScopeSnapshotValid(snapshot);
    const BOOL global = available && [snapshot[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:@YES];
    for (NSMenuItem *root in [state[@"roots"] allObjects]) {
        root.enabled = available;
        root.toolTip = available ? nil : ([state[@"loading"] boolValue] || [state[@"writing"] boolValue])
            ? @"正在读取或保存语音设置…" : @"无法读取语音设置，请重试。";
        for (NSMenuItem *item in root.submenu.itemArray) {
            item.enabled = available;
            item.state = available && global == (item.tag == 1) ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

// 菜单保存成功后，所有此前开始的宿主偏好读取都不能再预热范围缓存。
static inline uint64_t MSIMEGlobalVoiceScopeReadEpoch(NSString *directory) {
    return [MSIMEVoiceScopeState(directory, YES)[@"epoch"] unsignedLongLongValue];
}

// 工作开始后缓存未变时，磁盘返回值也可能来自重置后的较小修订；否则保留已接受的新快照。
static inline void MSIMEAcceptGlobalVoiceScopeResult(NSMutableDictionary *state, NSDictionary *baseline,
    NSDictionary *snapshot) {
    NSDictionary *current = state[@"snapshot"];
    if (!current || current == baseline || [current isEqual:baseline] ||
        (MSIMEVoiceScopeSnapshotValid(snapshot) &&
         [snapshot[@"revision"] compare:current[@"revision"]] != NSOrderedAscending)) {
        // 刷新也能确认文档重置或失效；旧文档的宿主读取不能在此后重新预热旧范围。
        if (!MSIMEVoiceScopeSnapshotValid(snapshot) ||
            (current && [snapshot[@"revision"] compare:current[@"revision"]] == NSOrderedAscending))
            state[@"epoch"] = @([state[@"epoch"] unsignedLongLongValue] + 1);
        if (MSIMEVoiceScopeSnapshotValid(snapshot)) state[@"snapshot"] = snapshot;
        else [state removeObjectForKey:@"snapshot"];
    }
}

// 复用宿主已接受的偏好读取，通常在第一次打开菜单之前就备好快照；迟到的旧读取不能覆盖新保存。
static inline void MSIMERecordGlobalVoiceScopeSnapshot(NSString *directory, NSDictionary *snapshot, uint64_t epoch) {
    if (!MSIMEVoiceScopeSnapshotValid(snapshot)) return;
    NSMutableDictionary *state = MSIMEVoiceScopeState(directory, YES);
    NSDictionary *current = state[@"snapshot"];
    if (state && epoch == [state[@"epoch"] unsignedLongLongValue] &&
        (!current || [snapshot[@"revision"] compare:current[@"revision"]] != NSOrderedAscending)) {
        state[@"snapshot"] = snapshot;
        MSIMEUpdateVoiceScopeMenus(state);
    }
}

static inline NSMenuItem *MSIMEGlobalVoiceScopeMenuForDirectory(NSString *directory, id target, SEL action,
    MSIMEVoiceScopeLoad load = nil) {
    NSMutableDictionary *state = MSIMEVoiceScopeState(directory, YES);
    NSDictionary *baseline = state[@"snapshot"];
    NSMenuItem *root = MSIMEGlobalVoiceScopeMenuItem(baseline, target, action);
    if (!state) return root;
    [state[@"roots"] addObject:root];
    // 跨进程菜单只携带可编码的目录字符串；可变状态与弱引用留在输入法进程内。
    for (NSMenuItem *choice in root.submenu.itemArray) choice.representedObject = [directory copy];
    if (![state[@"loading"] boolValue]) {
        state[@"loading"] = @YES;
        MSIMEVoiceScopeLoad reader = load ?: ^NSDictionary *(NSString *path) { return MSIMELoadGlobalVoiceScope(path); };
        NSString *path = [directory copy];
        dispatch_async(MSIMEGlobalVoiceScopeQueue(), ^{
            NSDictionary *snapshot = reader(path);
            dispatch_async(dispatch_get_main_queue(), ^{
                state[@"loading"] = @NO;
                MSIMEAcceptGlobalVoiceScopeResult(state, baseline, snapshot);
                MSIMEUpdateVoiceScopeMenus(state);
            });
        });
    }
    MSIMEUpdateVoiceScopeMenus(state);
    return root;
}

static inline void MSIMESelectGlobalVoiceScope(id sender, dispatch_block_t didSave,
    dispatch_block_t failed, MSIMEVoiceScopeLoad load = nil, MSIMEVoiceScopeSave save = nil) {
    // 系统输入菜单经 IMK 字典派发，普通 AppKit 菜单直接传 NSMenuItem；两种入口都严格校验。
    id selected = [sender isKindOfClass:NSDictionary.class] ? sender[kIMKCommandMenuItemName] : sender;
    if (![selected isKindOfClass:NSMenuItem.class]) return;
    NSMenuItem *item = selected;
    NSString *directory = item.representedObject;
    NSMutableDictionary *state = MSIMEVoiceScopeState(directory, NO);
    if (!state || [state[@"writing"] boolValue] || (item.tag != 0 && item.tag != 1)) return;
    NSDictionary *baseline = state[@"snapshot"];
    state[@"writing"] = @YES;
    MSIMEUpdateVoiceScopeMenus(state);
    const BOOL enabled = item.tag == 1;
    MSIMEVoiceScopeLoad reader = load ?: ^NSDictionary *(NSString *path) { return MSIMELoadGlobalVoiceScope(path); };
    MSIMEVoiceScopeSave writer = save ?: ^NSDictionary *(NSString *path, uint64_t revision, NSDictionary *snapshot) {
        return [MSIMEClientSession savePreferencesInDirectory:path expectedRevision:revision snapshot:snapshot error:nil];
    };
    dispatch_async(MSIMEGlobalVoiceScopeQueue(), ^{
        __block NSDictionary *saved = nil;
        const BOOL success = MSIMESaveGlobalVoiceScope(enabled, ^{ return reader(directory); },
            ^NSDictionary *(uint64_t revision, NSDictionary *snapshot) {
                saved = writer(directory, revision, snapshot);
                return saved;
            });
        dispatch_async(dispatch_get_main_queue(), ^{
            state[@"writing"] = @NO;
            if (success) state[@"epoch"] = @([state[@"epoch"] unsignedLongLongValue] + 1);
            MSIMEAcceptGlobalVoiceScopeResult(state, baseline, success ? saved : nil);
            MSIMEUpdateVoiceScopeMenus(state);
            if (success) { MSIMENotifyGlobalVoicePreferencesChanged(); if (didSave) didSave(); }
            else if (failed) failed();
        });
    });
}
