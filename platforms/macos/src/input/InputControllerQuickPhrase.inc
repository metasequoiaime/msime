// 由 InputController.mm 的实现区引入，临时 K 模式不写回中英文偏好。
- (BOOL)quickPhraseModeActive {
    return [_view[@"local_mode"] isEqual:@"quick_phrase"];
}
- (BOOL)handleQuickPhraseHotkey:(NSEvent *)event {
    if (!MSIMEToggleChordMatches(_appearance.quickPhraseToggleChord, event) ||
        ![_appearance localModeEnabled:@"quick_phrase"] || [self secureEventInputActive]) return NO;
    // 按住入口时吞掉已进入模式的重复事件，避免快捷键透传前把 K 上屏。
    if (event.isARepeat) return [self quickPhraseModeActive];
    if ([_view[@"editing_text"] length] || [_view[@"phrase_prefix"] length]) return NO;
    if (!_session || _focusPending) [self prepareSession];
    if (!_session || (MSIMEStrictBoolean(_view[@"dedicated_english"]) && !_managedEnglishComposition)) return NO;
    if (![self setManagedEnglishEnabled:NO]) return YES;
    NSDictionary *transition = [_session command:MSIME_ENTER_QUICK_PHRASE error:nil];
    if (![transition[@"handled"] isEqual:@YES]) return NO;
    [self apply:transition];
    return YES;
}

- (void)addQuickPhrasePriorityToMenu:(NSMenu *)menu {
    NSString *directory = [self initialPreferencesDirectory];
    NSDictionary *snapshot = directory ? [self readPreferencesSnapshotInDirectory:directory error:nil] : nil;
    NSDictionary *mixed = snapshot[@"preferences"][@"mixed_input"];
    if (!mixed || [mixed[@"enabled"] isEqual:@NO] || [mixed[@"quick_phrase"] isEqual:@NO]) return;
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:@"快捷短语置顶"
        action:@selector(selectQuickPhrasePriority:) keyEquivalent:@""];
    item.target = self;
    item.representedObject = directory;
    item.state = [mixed[@"quick_phrase_first"] isEqual:@YES] ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:item];
}

- (void)selectQuickPhrasePriority:(id)sender {
    id selected = [sender isKindOfClass:NSDictionary.class] ? sender[kIMKCommandMenuItemName] : sender;
    if (![selected isKindOfClass:NSMenuItem.class]) return;
    NSMenuItem *item = selected;
    NSString *directory = item.representedObject;
    BOOL enabled = item.state != NSControlStateValueOn;
    __weak MSIMEInputController *weakSelf = self;
    // 文件锁和持久化在后台进行；版本冲突后重读文档，只修改这一个字段。
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        BOOL saved = NO;
        for (int attempt = 0; attempt < 2 && !saved; ++attempt) {
            NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
            uint64_t revision = 0;
            if (!MSIMEStrictUnsignedInteger(snapshot[@"revision"], &revision)) break;
            NSMutableDictionary *preferences = [snapshot[@"preferences"] mutableCopy];
            NSMutableDictionary *mixed = [preferences[@"mixed_input"] mutableCopy];
            if (!mixed) break;
            mixed[@"quick_phrase_first"] = @(enabled);
            preferences[@"mixed_input"] = mixed;
            NSMutableDictionary *updated = [snapshot mutableCopy];
            updated[@"preferences"] = preferences;
            saved = [MSIMEClientSession savePreferencesInDirectory:directory
                expectedRevision:revision snapshot:updated error:nil] != nil;
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            if (saved) [weakSelf reloadPreferences];
            else [weakSelf presentHUDText:@"无法保存快捷短语设置" announcement:@"无法保存快捷短语设置"];
        });
    });
}
