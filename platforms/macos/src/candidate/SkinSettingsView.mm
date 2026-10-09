#import "SkinSettingsView.h"
#import "../settings/AppearancePreferences.h"
#import "../settings/SettingsLayout.h"
#import "CandidateSkinPreviewView.h"
#import "CandidateSkinAppearance.h"

#include "CandidateSkin.h"

#include <algorithm>

namespace
{
NSTextField *Label(NSString *text, CGFloat size, NSFontWeight weight, NSColor *color)
{
    NSTextField *label = [NSTextField labelWithString:text];
    label.font = [NSFont systemFontOfSize:size weight:weight];
    label.textColor = color;
    label.translatesAutoresizingMaskIntoConstraints = NO;
    return label;
}

// The catalog carries ids, titles and a mode, not prose, so a card says what kind of theme it is rather than repeating a per-theme description kept here.
NSString *ThemeDescription(const metasequoia::mac::ThemeCatalogEntry &entry)
{
    if (entry.id == "custom")
    {
        return @"在底色上叠加自己的配色，或使用下方的外部皮肤";
    }
    if (entry.appearance == "dark")
    {
        return @"固定深色的候选窗、悬浮工具栏与菜单配色";
    }
    if (entry.appearance == "light")
    {
        return @"固定浅色的候选窗、悬浮工具栏与菜单配色";
    }
    return @"跟随系统明暗，使用 macOS 原生配色";
}

NSString *JoinedSkinValues(const std::vector<std::string> &values)
{
    NSMutableString *result = [NSMutableString string];
    for (size_t index = 0; index < values.size(); ++index)
    {
        if (index > 0) [result appendString:@"/"];
        [result appendString:@(values[index].c_str())];
    }
    return result;
}
} // namespace

/// Scroll views lay an unflipped document view out from the bottom, which parks the list of skins
/// against the bottom edge with its first card out of sight above.
@interface MetasequoiaSkinDocumentView : NSView
@end
@implementation MetasequoiaSkinDocumentView
- (BOOL)isFlipped { return YES; }
@end

@interface MetasequoiaSkinSwitch : NSSwitch
@end
@implementation MetasequoiaSkinSwitch
- (void)mouseDown:(NSEvent *)event
{
    if (self.state == NSControlStateValueOn)
    {
        [self sendAction:self.action to:self.target];
        return;
    }
    [super mouseDown:event];
}
- (void)performClick:(id)sender
{
    if (self.state == NSControlStateValueOn)
    {
        [self sendAction:self.action to:self.target];
        return;
    }
    [super performClick:sender];
}
@end

@implementation MetasequoiaSkinSettingsView
{
    NSStackView *_document;
    NSView *_externalCards;
    NSLayoutConstraint *_emptyExternalHeight;
    NSTextField *_directoryLabel;
    NSTextField *_emptyLabel;
    NSTextField *_diagnosticsLabel;
    NSMutableArray<NSSwitch *> *_switches;
    NSMutableArray<MetasequoiaCandidatePreviewView *> *_previews;
    NSMutableArray<NSButton *> *_themeButtons;
    NSMutableArray<NSTextField *> *_titles;
    NSMutableArray<NSString *> *_skinIds;
    NSMutableArray<NSString *> *_skinNames;
    NSMutableArray<NSNumber *> *_skinCompatibility;
    /// The first _themeCardCount cards are the global themes; the rest are external packages.
    NSUInteger _themeCardCount;
    NSButton *_detachSkinButton;
    /// 浅色、深色模式各用哪款外部皮肤。
    NSTextField *_slotSummaryLabel;
    BOOL _didScrollToTop;
}

- (instancetype)initWithFrame:(NSRect)frameRect
{
    return [self initWithFrame:frameRect preferences:MSIMEAppearancePreferences.sharedPreferences];
}

- (instancetype)initWithFrame:(NSRect)frameRect preferences:(MSIMEAppearancePreferences *)preferences
{
    self = [super initWithFrame:frameRect];
    if (self == nil)
    {
        return nil;
    }
    self.translatesAutoresizingMaskIntoConstraints = NO;
    _preferences = preferences;
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(preferencesChanged:)
                                              name:MSIMEAppearanceDidChangeNotification object:preferences];
    self.accessibilityLabel = @"皮肤设置页";
    _switches = [NSMutableArray array];
    _previews = [NSMutableArray array];
    _themeButtons = [NSMutableArray array];
    _titles = [NSMutableArray array];
    _skinIds = [NSMutableArray array];
    _skinNames = [NSMutableArray array];
    _skinCompatibility = [NSMutableArray array];

    // The page opens on its summary, the way every other page of the settings window does. The 20pt
    // 皮肤 heading that used to sit above it said what the toolbar title and the selected sidebar
    // row both already say.
    NSTextField *summary =
        Label(@"选择全局主题，或从本机目录加载外部皮肤。外部皮肤会作为自定义主题使用。", 13.0, NSFontWeightRegular, [NSColor secondaryLabelColor]);
    summary.maximumNumberOfLines = 2;

    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    scroll.translatesAutoresizingMaskIntoConstraints = NO;
    scroll.hasVerticalScroller = YES;
    scroll.hasHorizontalScroller = NO;
    scroll.drawsBackground = NO;
    scroll.borderType = NSNoBorder;
    scroll.autohidesScrollers = YES;

    _document = [NSStackView stackViewWithViews:@[]];
    _document.orientation = NSUserInterfaceLayoutOrientationVertical;
    _document.alignment = NSLayoutAttributeLeading;
    _document.spacing = 16.0;
    _document.edgeInsets =
        NSEdgeInsetsMake(0.0, msime::mac::layout::kPageMargin, 20.0, msime::mac::layout::kPageMargin);
    _document.translatesAutoresizingMaskIntoConstraints = NO;
    // The stack goes inside a flipped container rather than being the document view itself: an
    // unflipped document view is laid out from the bottom, so the page opens showing the last skin
    // in the list with the first one above the visible area.
    NSView *documentContainer = [[MetasequoiaSkinDocumentView alloc] initWithFrame:NSZeroRect];
    documentContainer.translatesAutoresizingMaskIntoConstraints = NO;
    scroll.documentView = documentContainer;
    [documentContainer addSubview:_document];
    [NSLayoutConstraint activateConstraints:@[
        [documentContainer.topAnchor constraintEqualToAnchor:scroll.contentView.topAnchor],
        [documentContainer.leadingAnchor constraintEqualToAnchor:scroll.contentView.leadingAnchor],
        [documentContainer.widthAnchor constraintEqualToAnchor:scroll.contentView.widthAnchor],
        [_document.topAnchor constraintEqualToAnchor:documentContainer.topAnchor],
        [_document.leadingAnchor constraintEqualToAnchor:documentContainer.leadingAnchor],
        [_document.trailingAnchor constraintEqualToAnchor:documentContainer.trailingAnchor],
        [_document.bottomAnchor constraintEqualToAnchor:documentContainer.bottomAnchor],
    ]];

    [self addSubview:summary];
    [self addSubview:scroll];
    [NSLayoutConstraint activateConstraints:@[
        // The page margins are the ones every other page of the settings window uses, so that
        // landing on this one does not shift the summary and the cards under the pointer.
        [summary.leadingAnchor constraintEqualToAnchor:self.leadingAnchor constant:msime::mac::layout::kPageMargin],
        [summary.trailingAnchor constraintEqualToAnchor:self.trailingAnchor constant:-msime::mac::layout::kPageMargin],
        [summary.topAnchor constraintEqualToAnchor:self.topAnchor constant:msime::mac::layout::kPageMargin],
        [scroll.leadingAnchor constraintEqualToAnchor:self.leadingAnchor],
        [scroll.trailingAnchor constraintEqualToAnchor:self.trailingAnchor],
        [scroll.topAnchor constraintEqualToAnchor:summary.bottomAnchor constant:16.0],
        [scroll.bottomAnchor constraintEqualToAnchor:self.bottomAnchor],
    ]];

    for (const metasequoia::mac::ThemeCatalogEntry &entry : metasequoia::mac::ThemeCatalog())
    {
        [self addSection:[self makeCardForId:@(entry.id.c_str())
                                        name:@(entry.title.c_str())
                                 description:ThemeDescription(entry)
                                compatible:YES]];
    }
    _themeCardCount = _skinIds.count;

    NSTextField *externalTitle = Label(@"外部皮肤", 13.0, NSFontWeightSemibold, [NSColor secondaryLabelColor]);
    NSTextField *externalHelp =
        Label(@"把包含 skin.toml 的皮肤文件夹复制到下面的目录，然后刷新。浅色底的皮肤用于浅色模式，深色底的皮肤用于深色模式，跟随系统的皮肤两种模式都用；浅色、深色模式可以各用一款。",
              13.0, NSFontWeightRegular, [NSColor secondaryLabelColor]);
    externalHelp.maximumNumberOfLines = 3;
    _directoryLabel = Label(@"", 12.0, NSFontWeightRegular, [NSColor secondaryLabelColor]);
    _directoryLabel.accessibilityLabel = @"外部皮肤目录";
    _directoryLabel.selectable = YES;
    NSButton *open = [NSButton buttonWithTitle:@"打开目录" target:self action:@selector(openDirectory:)];
    open.bezelStyle = NSBezelStyleRounded;
    open.accessibilityLabel = @"打开皮肤目录";
    NSButton *refresh = [NSButton buttonWithTitle:@"刷新皮肤" target:self action:@selector(reload)];
    refresh.bezelStyle = NSBezelStyleRounded;
    refresh.accessibilityLabel = @"刷新皮肤";
    _detachSkinButton = [NSButton buttonWithTitle:@"不使用外部皮肤" target:self action:@selector(detachExternalSkin:)];
    _detachSkinButton.bezelStyle = NSBezelStyleRounded;
    _detachSkinButton.accessibilityLabel = @"自定义主题不使用外部皮肤";
    NSStackView *actions = [NSStackView stackViewWithViews:@[ open, refresh, _detachSkinButton ]];
    actions.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    actions.spacing = 8.0;
    NSBox *externalHeader = [[NSBox alloc] initWithFrame:NSZeroRect];
    MSIMEConfigureCard(externalHeader);
    externalHeader.accessibilityLabel = @"外部皮肤卡片";
    _slotSummaryLabel = Label(@"", 13.0, NSFontWeightRegular, [NSColor labelColor]);
    _slotSummaryLabel.accessibilityLabel = @"浅色与深色模式使用的外部皮肤";
    _slotSummaryLabel.maximumNumberOfLines = 2;
    NSStackView *headerStack =
        [NSStackView stackViewWithViews:@[ externalTitle, externalHelp, _slotSummaryLabel, _directoryLabel, actions ]];
    headerStack.orientation = NSUserInterfaceLayoutOrientationVertical;
    headerStack.alignment = NSLayoutAttributeLeading;
    headerStack.spacing = 6.0;
    headerStack.translatesAutoresizingMaskIntoConstraints = NO;
    [externalHeader addSubview:headerStack];
    [NSLayoutConstraint activateConstraints:@[
        [headerStack.leadingAnchor constraintEqualToAnchor:externalHeader.leadingAnchor constant:16.0],
        [headerStack.trailingAnchor constraintEqualToAnchor:externalHeader.trailingAnchor constant:-16.0],
        [headerStack.topAnchor constraintEqualToAnchor:externalHeader.topAnchor constant:12.0],
        [headerStack.bottomAnchor constraintEqualToAnchor:externalHeader.bottomAnchor constant:-12.0],
        [actions.trailingAnchor constraintLessThanOrEqualToAnchor:headerStack.trailingAnchor],
    ]];
    [self addSection:externalHeader];

    _externalCards = [[NSView alloc] initWithFrame:NSZeroRect];
    _externalCards.translatesAutoresizingMaskIntoConstraints = NO;
    _emptyExternalHeight = [_externalCards.heightAnchor constraintEqualToConstant:0];
    [self addSection:_externalCards];

    _emptyLabel =
        Label(@"尚未扫描。点击“刷新皮肤”读取皮肤目录。", 13.0, NSFontWeightRegular, [NSColor secondaryLabelColor]);
    _emptyLabel.accessibilityLabel = @"外部皮肤空状态";
    [self addSection:_emptyLabel];

    _diagnosticsLabel = Label(@"", 12.0, NSFontWeightRegular, [NSColor systemOrangeColor]);
    _diagnosticsLabel.accessibilityLabel = @"皮肤扫描诊断";
    _diagnosticsLabel.maximumNumberOfLines = 8;
    [self addSection:_diagnosticsLabel];

    [self reload];
    return self;
}

- (void)dealloc
{
    [NSNotificationCenter.defaultCenter removeObserver:self];
}

- (void)preferencesChanged:(NSNotification *)notification
{
    (void)notification;
    for (MetasequoiaCandidatePreviewView *preview in _previews) [preview reloadPreview];
    [self refreshSelection];
}

- (void)viewDidChangeEffectiveAppearance
{
    [super viewDidChangeEffectiveAppearance];
    [self refreshCardChrome];
}

- (void)addSection:(NSView *)view
{
    view.translatesAutoresizingMaskIntoConstraints = NO;
    [_document addArrangedSubview:view];
    // A card spans the stack minus the margin the stack insets it by on either side.
    [view.widthAnchor constraintEqualToAnchor:_document.widthAnchor
                                     constant:-2.0 * msime::mac::layout::kPageMargin]
        .active = YES;
}

- (NSView *)makeCardForId:(NSString *)skinId
                     name:(NSString *)name
              description:(NSString *)description
              compatible:(BOOL)compatible
{
    NSBox *card = [[NSBox alloc] initWithFrame:NSZeroRect];
    MSIMEConfigureCard(card);
    card.accessibilityLabel = [name stringByAppendingString:@"皮肤卡片"];
    NSTextField *title = Label(name, 15.0, NSFontWeightSemibold, [NSColor labelColor]);
    title.accessibilityLabel = [name stringByAppendingString:@"标题"];
    NSTextField *summary = Label(description, 13.0, NSFontWeightRegular, [NSColor secondaryLabelColor]);
    summary.maximumNumberOfLines = 2;
    NSSwitch *enable = [[MetasequoiaSkinSwitch alloc] initWithFrame:NSZeroRect];
    enable.identifier = skinId;
    enable.target = self;
    enable.action = @selector(enableSkin:);
    enable.enabled = compatible;
    enable.accessibilityLabel = [@"启用" stringByAppendingString:name];
    if (!compatible) enable.accessibilityValue = @"当前布局或明暗模式不受支持";
    NSButton *theme = [NSButton buttonWithTitle:@"预览浅色" target:self action:@selector(toggleCardTheme:)];
    theme.bezelStyle = NSBezelStyleRounded;
    theme.identifier = skinId;
    theme.accessibilityLabel = [name stringByAppendingString:@"预览明暗"];
    MetasequoiaCandidatePreviewView *preview = [[MetasequoiaCandidatePreviewView alloc] initWithFrame:NSZeroRect];
    preview.preferences = _preferences;
    [preview setShowsLayoutShowcase:YES];
    [preview setPreviewSkinId:skinId];
    preview.accessibilityLabel = [name stringByAppendingString:@"预览"];
    NSStackView *text = [NSStackView stackViewWithViews:@[ title, summary ]];
    text.orientation = NSUserInterfaceLayoutOrientationVertical;
    text.alignment = NSLayoutAttributeLeading;
    text.spacing = 4.0;
    NSStackView *actions = [NSStackView stackViewWithViews:@[ enable, theme ]];
    actions.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    actions.spacing = 8.0;
    NSStackView *header = [NSStackView stackViewWithViews:@[ text, actions ]];
    header.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    header.alignment = NSLayoutAttributeTop;
    header.distribution = NSStackViewDistributionFill;
    NSStackView *stack = [NSStackView stackViewWithViews:@[ header, preview ]];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = 10.0;
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [card addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:card.leadingAnchor constant:16.0],
        [stack.trailingAnchor constraintEqualToAnchor:card.trailingAnchor constant:-16.0],
        [stack.topAnchor constraintEqualToAnchor:card.topAnchor constant:12.0],
        [stack.bottomAnchor constraintEqualToAnchor:card.bottomAnchor constant:-12.0],
        [preview.widthAnchor constraintEqualToAnchor:stack.widthAnchor],
        [header.widthAnchor constraintEqualToAnchor:stack.widthAnchor],
        [text.trailingAnchor constraintLessThanOrEqualToAnchor:actions.leadingAnchor constant:-12.0],
    ]];
    [_skinIds addObject:skinId];
    [_skinNames addObject:name];
    [_skinCompatibility addObject:@(compatible)];
    [_switches addObject:enable];
    [_previews addObject:preview];
    [_themeButtons addObject:theme];
    [_titles addObject:title];
    return card;
}

- (void)refreshSelection
{
    [self refreshCardChrome];
}

- (void)enableSkin:(NSSwitch *)sender
{
    NSString *skinId = sender.identifier;
    if (skinId.length == 0)
    {
        return;
    }
    sender.state = NSControlStateValueOn;
    if (metasequoia::mac::IsGlobalThemeId(skinId.UTF8String))
    {
        // 自定义卡片按现状选中自定义主题，两个槽位里的皮肤包一并保留（THEME_CONTRACT §5）；只有停用外部皮肤卡片或「不使用外部皮肤」才去掉包。
        _preferences.globalTheme = skinId;
        return;
    }
    // 外部皮肤按明暗各占一个槽位，可以同时启用两款：已启用的再点一次就停用，只清放着它的槽位。
    if ([self externalSkinInUse:skinId])
    {
        sender.state = NSControlStateValueOff;
        [_preferences removeCustomCandidateSkin:skinId];
        return;
    }
    const std::filesystem::path root = _preferences.skinsRoot.fileSystemRepresentation ?: "";
    const auto package = msime::mac::LoadSkinPackage(root, skinId.UTF8String);
    [_preferences selectExternalSkin:skinId base:package ? @(package->base.c_str()) : @"system"];
}

/// 自定义主题正在用、并且这款包放在浅色或深色槽位里。
- (BOOL)externalSkinInUse:(NSString *)skinId
{
    return [_preferences.globalTheme isEqual:@"custom"] &&
           ([_preferences.customCandidateSkin isEqual:skinId] || [_preferences.customCandidateSkinDark isEqual:skinId]);
}

/// 一种明暗用的外部皮肤：深色模式先取深色槽位、空着时取浅色槽位，包只在它 base 的明暗下画（与 msime_client_resolve_theme 的规则相同，不看布局）。没有时为 nil。
- (NSString *)externalSkinNameForDark:(BOOL)dark
{
    NSString *skinId = dark ? (_preferences.customCandidateSkinDark ?: _preferences.customCandidateSkin)
                            : _preferences.customCandidateSkin;
    if (skinId == nil) return nil;
    const std::filesystem::path root = _preferences.skinsRoot.fileSystemRepresentation ?: "";
    const auto package = msime::mac::LoadSkinPackage(root, skinId.UTF8String);
    // 目录里已经没有这款包：照样报出它的 id，让用户知道槽位里还放着它。
    if (!package) return skinId;
    const msime::mac::SkinSlot slot = msime::mac::SkinSlotOfBase(package->base);
    if (slot == (dark ? msime::mac::SkinSlot::light : msime::mac::SkinSlot::dark)) return nil;
    return @(package->name.c_str());
}

- (void)refreshSlotSummary
{
    const BOOL configured = _preferences.customCandidateSkin != nil || _preferences.customCandidateSkinDark != nil;
    if (![_preferences.globalTheme isEqual:@"custom"] || !configured)
    {
        _slotSummaryLabel.stringValue = @"";
        _slotSummaryLabel.hidden = YES;
        return;
    }
    NSString *(^describe)(NSString *, NSString *) = ^NSString *(NSString *mode, NSString *name) {
        return name ? [NSString stringWithFormat:@"%@使用「%@」", mode, name]
                    : [NSString stringWithFormat:@"%@不使用外部皮肤", mode];
    };
    _slotSummaryLabel.stringValue = [NSString stringWithFormat:@"%@，%@。", describe(@"浅色模式", [self externalSkinNameForDark:NO]),
                                                                       describe(@"深色模式", [self externalSkinNameForDark:YES])];
    _slotSummaryLabel.hidden = NO;
}

- (void)detachExternalSkin:(id)sender
{
    (void)sender;
    [_preferences clearCustomCandidateSkin];
}

/// Whether a package can be selected in the current layout. A package over a built-in base is drawn in that base's mode, so the host mode does not rule it out; over a system base it is drawn in the mode the candidate window resolves for a system base (the 候选窗主题 / 主题 light-dark choice, else the system's), which its manifest has to list. This is the React host's rule (external-skins.tsx) and THEME_CONTRACT §5.
- (BOOL)packageIsCompatible:(const msime::mac::SkinPackage &)package
{
    const std::string layout = _preferences.vertical ? "vertical" : "horizontal";
    for (const metasequoia::mac::ThemeCatalogEntry &entry : metasequoia::mac::ThemeCatalog())
    {
        if (entry.id == package.base && !entry.appearance.empty())
            return std::find(package.layouts.begin(), package.layouts.end(), layout) != package.layouts.end();
    }
    NSAppearance *appearance = _preferences.systemBaseCandidateAppearanceOverride ?: self.effectiveAppearance;
    NSString *host = MetasequoiaAppearanceIsDark(appearance) ? @"dark" : @"light";
    return msime::mac::SupportsSkin(package, layout, host.UTF8String);
}

- (void)toggleCardTheme:(NSButton *)sender
{
    NSUInteger index = [_themeButtons indexOfObject:sender];
    if (index == NSNotFound)
    {
        return;
    }
    [_previews[index] toggleForcedTheme];
    [self refreshCardChrome];
}

- (void)openDirectory:(id)sender
{
    (void)sender;
    NSURL *directory = _preferences.skinsRoot;
    if (directory == nil)
    {
        return;
    }
    BOOL created = [[NSFileManager defaultManager] createDirectoryAtURL:directory
                             withIntermediateDirectories:YES
                                              attributes:nil
                                                   error:nil];
    if (!created) {
        _diagnosticsLabel.stringValue = @"无法创建皮肤目录。";
        _diagnosticsLabel.hidden = NO;
        return;
    }
    BOOL opened = self.directoryOpener ? self.directoryOpener(directory) : [[NSWorkspace sharedWorkspace] openURL:directory];
    [self reload];
    if (!opened) {
        _diagnosticsLabel.stringValue = @"无法打开皮肤目录。";
        _diagnosticsLabel.hidden = NO;
    }
}

- (void)refreshCardChrome
{
    NSString *active = _preferences.globalTheme;
    const std::filesystem::path root = _preferences.skinsRoot.fileSystemRepresentation ?: "";
    _detachSkinButton.enabled = _preferences.customCandidateSkin != nil || _preferences.customCandidateSkinDark != nil;
    [self refreshSlotSummary];
    for (NSUInteger index = 0; index < _skinIds.count; ++index)
    {
        BOOL compatible = YES;
        BOOL selected = NO;
        if (index >= _themeCardCount)
        {
            auto package = msime::mac::LoadSkinPackage(root, _skinIds[index].UTF8String);
            compatible = package.has_value() && [self packageIsCompatible:*package];
            _skinCompatibility[index] = @(compatible);
            // 放在浅色或深色槽位里都算启用。
            selected = [self externalSkinInUse:_skinIds[index]];
        }
        else
        {
            // A theme card is on while its theme is the global theme; under a package the custom card stays on beside the package's card, since the package is drawn over the custom theme.
            selected = [_skinIds[index] isEqualToString:active];
        }
        _switches[index].state = selected ? NSControlStateValueOn : NSControlStateValueOff;
        _switches[index].enabled = compatible;
        _themeButtons[index].title = [_previews[index] forcedThemeButtonTitle];
        // A theme with a fixed mode looks the same in both, so there is nothing to preview in the other.
        _themeButtons[index].hidden = [_previews[index] previewHasFixedMode];
        _titles[index].stringValue = [NSString
            stringWithFormat:@"%@（%@）", _skinNames[index], [_previews[index] previewUsesDark] ? @"Dark" : @"Light"];
        _previews[index].needsDisplay = YES;
    }
}

- (void)clearExternalCards
{
    while (_skinIds.count > _themeCardCount)
    {
        [_skinIds removeLastObject];
        [_skinNames removeLastObject];
        [_skinCompatibility removeLastObject];
        [_switches removeLastObject];
        [_previews removeLastObject];
        [_themeButtons removeLastObject];
        [_titles removeLastObject];
    }
    for (NSView *child in [_externalCards.subviews copy])
    {
        [child removeFromSuperview];
    }
}

- (void)reload
{
    [self clearExternalCards];
    [_preferences reloadSkins];
    const std::filesystem::path root = _preferences.skinsRoot.fileSystemRepresentation ?: "";
    NSString *path = @(root.string().c_str());
    if ([path hasPrefix:NSHomeDirectory()])
    {
        path = [@"~" stringByAppendingString:[path substringFromIndex:NSHomeDirectory().length]];
    }
    _directoryLabel.stringValue = path.length > 0 ? path : @"~/Library/Application Support/metasequoiaime/skins";

    const metasequoia::mac::SkinCatalog catalog = metasequoia::mac::ScanSkinCatalog(root);
    NSMutableArray<NSView *> *cards = [NSMutableArray array];
    for (const metasequoia::mac::SkinPackage &package : catalog.packages)
    {
        NSString *description = package.description.empty()
                                    ? [NSString stringWithFormat:@"基于%s", msime::mac::ThemeTitle(package.base).c_str()]
                                    : @(package.description.c_str());
        const BOOL compatible = [self packageIsCompatible:package];
        if (!compatible) {
            description = [NSString stringWithFormat:@"当前布局或明暗模式不受支持（%@，%@）",
                                                     JoinedSkinValues(package.layouts), JoinedSkinValues(package.themes)];
        }
        [cards addObject:[self makeCardForId:@(package.id.c_str())
                                        name:@(package.name.c_str())
                                 description:description
                                compatible:compatible]];
    }
    if (cards.count > 0)
    {
        _emptyExternalHeight.active = NO;
        NSStackView *stack = [NSStackView stackViewWithViews:cards];
        stack.orientation = NSUserInterfaceLayoutOrientationVertical;
        stack.alignment = NSLayoutAttributeLeading;
        stack.spacing = 16.0;
        stack.translatesAutoresizingMaskIntoConstraints = NO;
        [_externalCards addSubview:stack];
        [NSLayoutConstraint activateConstraints:@[
            [stack.leadingAnchor constraintEqualToAnchor:_externalCards.leadingAnchor],
            [stack.trailingAnchor constraintEqualToAnchor:_externalCards.trailingAnchor],
            [stack.topAnchor constraintEqualToAnchor:_externalCards.topAnchor],
            [stack.bottomAnchor constraintEqualToAnchor:_externalCards.bottomAnchor],
            [stack.widthAnchor constraintEqualToAnchor:_externalCards.widthAnchor],
        ]];
        for (NSView *card in cards)
        {
            [card.widthAnchor constraintEqualToAnchor:stack.widthAnchor].active = YES;
        }
    }
    _emptyExternalHeight.active = cards.count == 0;
    _emptyLabel.hidden = cards.count > 0;
    _emptyLabel.stringValue = cards.count > 0 ? @"" : @"没有发现外部皮肤。把皮肤文件夹放到目录中后点击“刷新皮肤”。";
    if (catalog.issues.empty())
    {
        _diagnosticsLabel.stringValue = @"";
        _diagnosticsLabel.hidden = YES;
    }
    else
    {
        NSMutableString *text = [NSMutableString stringWithFormat:@"已忽略 %zu 个无效皮肤目录", catalog.issues.size()];
        for (const metasequoia::mac::SkinIssue &issue : catalog.issues)
        {
            [text appendFormat:@"\n%s：%s", issue.folder.c_str(), issue.reason.c_str()];
        }
        _diagnosticsLabel.stringValue = text;
        _diagnosticsLabel.hidden = NO;
    }
    [self refreshCardChrome];
    [_document layoutSubtreeIfNeeded];
    [self scrollCardsToTop];
}

/// A stack view is not flipped, so a scroll view holding one opens showing its bottom — the last
/// skin in the list — and the first card is above the visible area. Harmless in the fixed 720pt
/// window this used to live in, visible as soon as it became a page that gets resized.
- (void)scrollCardsToTop
{
    NSScrollView *scroll = _document.enclosingScrollView;
    NSView *document = scroll.documentView;
    if (document == nil) return;
    const CGFloat top = document.isFlipped ? 0.0 : NSMaxY(document.frame) - NSHeight(scroll.contentView.bounds);
    [document scrollPoint:NSMakePoint(0.0, MAX(0.0, top))];
}

// Cards are built before the view has a superview, so nothing has a size yet and the scroll during
// reload has nothing to work with. The first move into a window is the first moment it does.
- (void)viewDidMoveToWindow
{
    [super viewDidMoveToWindow];
    if (self.window == nil || _didScrollToTop) return;
    _didScrollToTop = YES;
    dispatch_async(dispatch_get_main_queue(), ^{
      [self.window.contentView layoutSubtreeIfNeeded];
      [self scrollCardsToTop];
    });
}

@end
