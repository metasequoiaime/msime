#import "../../src/settings/AppearancePreferences.h"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <string>
#import "../settings/TestPreferenceSuite.h"

// 选中的皮肤包在原处被同名替换后，下一次检查要读到新包：MCP 的 `create_candidate_skin` 带 replace 换掉正在用的皮肤后，输入法不重启也要画新的装饰带。

static const char *const kSkinId = "synthetic-replace";

static void WritePng(const std::filesystem::path &path, NSInteger width, NSInteger height) {
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:width pixelsHigh:height
                                                                    bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
                                                                   colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    assert(bitmap);
    NSData *png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    assert(png.length > 0);
    assert([png writeToFile:@(path.c_str()) atomically:NO]);
}

static void WriteManifest(const std::filesystem::path &folder, int topInsetDip, int widthDip) {
    std::ofstream file(folder / "skin.toml");
    file << "schema_version = 1\nid = \"" << kSkinId << "\"\nname = \"Synthetic Replace\"\nversion = \"1\"\nbase = \"system\"\n"
         << "[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"dark\", \"light\"]\n"
         << "[candidate_window]\nmin_width_dip = 0\n[candidate_window.decoration]\nimage = \"decoration.png\"\n"
         << "top_inset_dip = " << topInsetDip << "\nwidth_dip = " << widthDip << "\n";
    assert(file.good());
}

static void WritePackage(const std::filesystem::path &folder, int topInsetDip, int widthDip, NSInteger imageWidth, NSInteger imageHeight) {
    std::filesystem::create_directories(folder);
    WriteManifest(folder, topInsetDip, widthDip);
    WritePng(folder / "decoration.png", imageWidth, imageHeight);
}

static NSSize DecorationPixels(MSIMEAppearancePreferences *preferences) {
    NSImageRep *rep = [preferences decorationImageForDark:NO].representations.firstObject;
    return rep ? NSMakeSize(rep.pixelsWide, rep.pixelsHigh) : NSZeroSize;
}

int main() {
    @autoreleasepool {
        char temporary[] = "/tmp/msime-skin-replace-test-XXXXXX";
        assert(mkdtemp(temporary));
        const std::filesystem::path root = std::filesystem::path(temporary) / "skins";
        const std::filesystem::path target = root / kSkinId;
        WritePackage(target, 26, 96, 192, 80);
        NSString *suite = [@"app.msime.test.skin-replace." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *preferences =
            [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
        [preferences selectExternalSkin:@(kSkinId) base:@"system"];
        assert([preferences resolvedSkinForDark:NO].candidateSkin == kSkinId);
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 26.0);
        assert(NSEqualSizes(DecorationPixels(preferences), NSMakeSize(192, 80)));
        // 什么都没变时不重新解析。
        assert(![preferences reloadReplacedSkinPackages]);

        // 照 client-services 的安装那样替换：新包写进暂存目录，旧目录改名让开，新目录 rename 进来，再删掉旧的。同名、同路径，内容全换。
        const std::filesystem::path staging = root / ".staging" / kSkinId;
        WritePackage(staging, 73, 132, 264, 152);
        std::filesystem::rename(target, root / ".replaced-synthetic-replace");
        std::filesystem::rename(staging, target);
        std::filesystem::remove_all(root / ".replaced-synthetic-replace");
        std::filesystem::remove_all(root / ".staging");
        // 偏好没有动，原有的解析结果还是旧包的：这就是输入法一直画旧装饰的原因。
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 26.0);
        assert([preferences reloadReplacedSkinPackages]);
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 73.0);
        assert([preferences resolvedSkinForDark:NO].decorationWidthDip == 132.0);
        assert([preferences resolvedSkinForDark:YES].decorationTopDip == 73.0);
        assert(NSEqualSizes(DecorationPixels(preferences), NSMakeSize(264, 152)));
        assert(![preferences reloadReplacedSkinPackages]);

        // 原地改写装饰图（目录 inode 不变）也算替换。
        WritePng(target / "decoration.png", 120, 60);
        assert([preferences reloadReplacedSkinPackages]);
        assert(NSEqualSizes(DecorationPixels(preferences), NSMakeSize(120, 60)));
        assert(![preferences reloadReplacedSkinPackages]);

        // 原地改写 skin.toml 同样。
        WriteManifest(target, 40, 132);
        assert([preferences reloadReplacedSkinPackages]);
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 40.0);

        // 包被删掉后回到没有装饰的解析结果，再装回来又能读到。
        std::filesystem::remove_all(target);
        assert([preferences reloadReplacedSkinPackages]);
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 0.0 && [preferences decorationImageForDark:NO] == nil);
        WritePackage(target, 73, 132, 264, 152);
        assert([preferences reloadReplacedSkinPackages]);
        assert([preferences resolvedSkinForDark:NO].decorationTopDip == 73.0);

        MSIMERemoveTestPreferenceSuite(defaults, suite);
        std::filesystem::remove_all(temporary);
    }
    return 0;
}
