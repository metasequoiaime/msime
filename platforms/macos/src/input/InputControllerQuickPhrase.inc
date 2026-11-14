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
