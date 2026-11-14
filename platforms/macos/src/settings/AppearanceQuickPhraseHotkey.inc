// 由 AppearancePreferences.mm 引入；共享文档与本机写入保持同一份当前值。
- (NSString *)quickPhraseHotkey {
    return _sharedQuickPhraseHotkey ?: [_defaults stringForKey:@"MSIMEClientQuickPhraseHotkey"] ?: @"";
}
- (void)setQuickPhraseHotkey:(NSString *)value {
    _sharedQuickPhraseHotkey = [value copy] ?: @"";
    [_defaults setObject:_sharedQuickPhraseHotkey forKey:@"MSIMEClientQuickPhraseHotkey"];
    NSMutableDictionary *keys = [_quickPhraseKeybindings mutableCopy] ?: [NSMutableDictionary dictionary];
    keys[@"enter_quick_phrase"] = _sharedQuickPhraseHotkey;
    _quickPhraseKeybindings = keys;
    _quickPhraseToggleChord = MSIMEDecodeToggleChord(MSIMEToggleHotkeyChords(keys)[@"quick_phrase"]);
    _quickPhraseHotkeyControl.recorder.binding = self.quickPhraseHotkey;
    [self preferencesChanged];
}
