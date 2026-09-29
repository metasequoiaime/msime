#import "CandidateSkinAppearance.h"

NSNotificationName const MetasequoiaCandidateSkinDidChangeNotification =
    @"MetasequoiaCandidateSkinDidChangeNotification";
// The same key MSIMEAppearancePreferences writes, so the retained panel and the toolbar fallback draw the theme the settings window chose.
static NSString *const kGlobalThemePreferenceKey = @"MSIMEClientGlobalTheme";

NSColor *MetasequoiaColorFromRgba(metasequoia::mac::Rgba color)
{
    return [NSColor colorWithSRGBRed:color.r green:color.g blue:color.b alpha:color.a];
}

BOOL MetasequoiaAppearanceIsDark(NSAppearance *appearance)
{
    NSAppearance *resolved = appearance;
    if (resolved == nil)
    {
        resolved = NSApp.effectiveAppearance;
    }
    if (resolved == nil)
    {
        resolved = NSAppearance.currentDrawingAppearance;
    }
    if (resolved == nil)
    {
        return NO;
    }
    NSString *match = [resolved bestMatchFromAppearancesWithNames:@[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]];
    return [match isEqualToString:NSAppearanceNameDarkAqua];
}

NSURL *MetasequoiaCandidateSkinsDirectoryURL(void)
{
    const std::filesystem::path path = metasequoia::mac::DefaultSkinsRoot();
    if (path.empty())
    {
        return nil;
    }
    return [NSURL fileURLWithPath:@(path.c_str()) isDirectory:YES];
}

NSString *MetasequoiaStoredGlobalTheme(void)
{
    NSString *value = [[NSUserDefaults standardUserDefaults] stringForKey:kGlobalThemePreferenceKey];
    return metasequoia::mac::IsGlobalThemeId(value.UTF8String ?: "") ? value : @"system";
}

void MetasequoiaSetStoredGlobalTheme(NSString *themeId)
{
    if (!metasequoia::mac::IsGlobalThemeId(themeId.UTF8String ?: ""))
    {
        return;
    }
    [[NSUserDefaults standardUserDefaults] setObject:themeId forKey:kGlobalThemePreferenceKey];
    [[NSNotificationCenter defaultCenter] postNotificationName:MetasequoiaCandidateSkinDidChangeNotification
                                                        object:themeId];
}

metasequoia::mac::CustomTheme MetasequoiaStoredCustomTheme(void)
{
    NSUserDefaults *defaults = [NSUserDefaults standardUserDefaults];
    auto read = [defaults](NSString *key) {
        NSString *value = [defaults stringForKey:key];
        return std::string(value.UTF8String ?: "");
    };
    metasequoia::mac::CustomTheme custom;
    const std::string base = read(@"MSIMEClientCustomThemeBase");
    custom.base = metasequoia::mac::IsThemeBaseId(base) ? base : "system";
    custom.candidateSkin = read(@"MSIMEClientCustomCandidateSkin");
    custom.candidateColors.text = read(@"MSIMEClientCandidateTextColor");
    custom.candidateColors.number = read(@"MSIMEClientCandidateNumberColor");
    custom.candidateColors.accent = read(@"MSIMEClientCandidateAccentColor");
    custom.candidateColors.selected = read(@"MSIMEClientCandidateSelectedColor");
    custom.candidateColors.hover = read(@"MSIMEClientCandidateHoverColor");
    custom.candidateColors.surface = read(@"MSIMEClientCandidateSurfaceColor");
    custom.candidateColors.border = read(@"MSIMEClientCandidateBorderColor");
    return custom;
}

metasequoia::mac::ResolvedSkin MetasequoiaResolveStoredTheme(BOOL dark, BOOL vertical)
{
    return metasequoia::mac::ResolveSkin(MetasequoiaStoredGlobalTheme().UTF8String, MetasequoiaStoredCustomTheme(), dark,
                                         vertical ? "vertical" : "horizontal", metasequoia::mac::DefaultSkinsRoot());
}
