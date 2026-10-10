#pragma once

#import <AppKit/AppKit.h>

#include "CandidateSkin.h"

FOUNDATION_EXPORT NSNotificationName const MetasequoiaCandidateSkinDidChangeNotification;

NSColor *MetasequoiaColorFromRgba(metasequoia::mac::Rgba color);
BOOL MetasequoiaAppearanceIsDark(NSAppearance *appearance);
NSURL *MetasequoiaCandidateSkinsDirectoryURL(void);
/// The global theme id stored in the standard defaults under the key the settings window writes (`MSIMEClientGlobalTheme`). An id outside the catalog reads as `system`.
NSString *MetasequoiaStoredGlobalTheme(void);
/// Stores a global theme id and posts MetasequoiaCandidateSkinDidChangeNotification. An id outside the catalog is ignored.
void MetasequoiaSetStoredGlobalTheme(NSString *themeId);
/// 设置窗口存下的 `custom_theme`：底色、浅色与深色两个槽位的候选皮肤和七个取色器。
metasequoia::mac::CustomTheme MetasequoiaStoredCustomTheme(void);
/// The stored global theme resolved for one mode and one candidate layout: a package is drawn only in the layouts its manifest declares.
metasequoia::mac::ResolvedSkin MetasequoiaResolveStoredTheme(BOOL dark, BOOL vertical);
