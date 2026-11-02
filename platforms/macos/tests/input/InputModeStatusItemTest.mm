#import "../../src/input/InputModeStatusItem.h"
#import "../../src/input/InputModeIdentifiers.h"

#include <cstdio>
#include <cstdlib>

// 菜单栏中/英状态项的图像（#10630）：切换中英文时图像必须跟着变，同时还要和旁边系统输入源那个光秃秃的模式字形区分开（#8548）。图像资源取仓库里的 resources 目录，Info.plist 用与 Info.plist.in 相同取值路径的合成字典，不往菜单栏放东西。

namespace {
void require(bool condition, const char *message) {
    if (condition) return;
    std::fprintf(stderr, "input-mode-status-item: %s\n", message);
    std::exit(1);
}

NSDictionary *ModeEntry(NSString *file) { return @{@"tsInputModeMenuIconFileKey": file}; }

// 按状态项的大小以 2 倍像素渲染成 RGBA，便于逐像素比较。模板图只看 alpha，所以只比 alpha 通道。
NSData *AlphaOf(NSImage *image) {
    const NSSize size = image.size;
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:(NSInteger)(size.width * 2) pixelsHigh:(NSInteger)(size.height * 2) bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:32];
    rep.size = size;
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
    [image drawInRect:NSMakeRect(0, 0, size.width, size.height) fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1];
    [NSGraphicsContext restoreGraphicsState];
    NSMutableData *alpha = [NSMutableData dataWithLength:(NSUInteger)(rep.pixelsWide * rep.pixelsHigh)];
    unsigned char *out = (unsigned char *)alpha.mutableBytes;
    for (NSInteger y = 0; y < rep.pixelsHigh; ++y)
        for (NSInteger x = 0; x < rep.pixelsWide; ++x) out[y * rep.pixelsWide + x] = rep.bitmapData[y * rep.bytesPerRow + x * 4 + 3];
    return alpha;
}

// 图像外沿一圈有没有着色：系统输入源的模式字形四周留白，状态项要靠这一圈和它区分。
bool HasInkOnEdge(NSData *alpha, NSInteger side) {
    const unsigned char *pixels = (const unsigned char *)alpha.bytes;
    NSInteger inked = 0;
    for (NSInteger i = 0; i < side; ++i) {
        if (pixels[side + i] > 128) ++inked;
        if (pixels[i * side + 1] > 128) ++inked;
    }
    return inked > side;
}
}  // namespace

int main() {
    @autoreleasepool {
        NSBundle *resources = [NSBundle bundleWithPath:@MSIME_MACOS_RESOURCES_DIR];
        require(resources != nil, "resources directory is not readable as a bundle");
        NSDictionary *full = @{
            @"tsInputMethodIconFileKey": @"MSIMEClientInputMethodMenuIcon.tiff",
            @"ComponentInputModeDict": @{@"tsInputModeListKey": @{
                MSIMEChineseInputModeID: ModeEntry(@"MSIMEClientInputMethodMenuIconChinese.tiff"),
                MSIMEShuangpinInputModeID: ModeEntry(@"MSIMEClientInputMethodMenuIconShuangpin.tiff"),
                MSIMEEnglishInputModeID: ModeEntry(@"MSIMEClientInputMethodMenuIconEnglish.tiff"),
            }},
        };

        NSImage *chinese = MSIMEInputModeStatusItemImage(full, MSIMEChineseInputModeID, resources);
        NSImage *english = MSIMEInputModeStatusItemImage(full, MSIMEEnglishInputModeID, resources);
        NSImage *shuangpin = MSIMEInputModeStatusItemImage(full, MSIMEShuangpinInputModeID, resources);
        require(chinese && english && shuangpin, "a declared mode has no status item image");
        require([chinese isTemplate] && [english isTemplate] && [shuangpin isTemplate], "status item images must be template images so the menu bar tints them");

        // #10630：中、英、双三个模式的图像必须彼此不同，否则切换中英文时菜单栏上看不出任何变化。
        NSData *chineseAlpha = AlphaOf(chinese);
        NSData *englishAlpha = AlphaOf(english);
        require(![chineseAlpha isEqualToData:englishAlpha], "Chinese and English modes draw the same status item image");
        require(![chineseAlpha isEqualToData:AlphaOf(shuangpin)], "Chinese and Shuangpin modes draw the same status item image");

        // #8548：状态项不能就是系统输入源那张模式字形。字形四周留白，状态项外沿要有一圈框。
        NSImage *bareGlyph = [resources imageForResource:@"MSIMEClientInputMethodMenuIconChinese.tiff"];
        require(bareGlyph != nil, "Chinese mode glyph is missing from resources");
        const NSInteger side = (NSInteger)(chinese.size.width * 2);
        require(chinese.size.width == chinese.size.height, "status item image must be square");
        require(HasInkOnEdge(chineseAlpha, side) && HasInkOnEdge(englishAlpha, side), "status item image has no frame setting it apart from the input source glyph");
        require(!HasInkOnEdge(AlphaOf(bareGlyph), (NSInteger)(bareGlyph.size.width * 2)), "the bare mode glyph unexpectedly reaches its edge; the frame check proves nothing");

        // 本版本没有声明的模式（五笔版把五笔放在 .Hans 上）与系统输入法菜单一致，画主模式的字形。
        NSImage *wubi = MSIMEInputModeStatusItemImage(full, MSIMEWubiInputModeID, resources);
        require(wubi && [AlphaOf(wubi) isEqualToData:chineseAlpha], "an undeclared mode should draw the main mode's glyph");

        // bundle 里找不到模式图标时退回产品 logo，状态项仍有图像可放。
        NSDictionary *missing = @{@"ComponentInputModeDict": @{@"tsInputModeListKey": @{MSIMEEnglishInputModeID: ModeEntry(@"DoesNotExist.tiff")}}};
        require(MSIMEInputModeStatusItemImage(missing, MSIMEEnglishInputModeID, resources) != nil, "a missing mode icon should fall back to the product logo");
    }
    std::puts("input-mode-status-item: ok");
    return 0;
}
