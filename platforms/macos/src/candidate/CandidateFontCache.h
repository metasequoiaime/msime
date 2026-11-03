#pragma once
#import <AppKit/AppKit.h>

// 将字体族名解析为候选字体使用的已安装描述符；没有同名已安装字体时返回 nil。CoreText 的族匹配会在每次候选绘制中多次执行，因此命中和未命中都按字体族名缓存，最多保留 128 项，并在字体集变化时清空。调用方每次绘制都会重新读取偏好，所以其他进程写入的字体或 fallback 变化无需触碰此缓存即可生效。可从任意线程调用。
NSFontDescriptor *MSIMEInstalledFontFamilyDescriptor(NSString *family);

// 同一个字体族里最接近 `weight` 的字面（#10748 的候选字重）。常规字重与 MSIMEInstalledFontFamilyDescriptor 完全相同；其他字重按族和字重一起缓存，族里没有这一档时得到 CoreText 选出的就近字面，不会换成别的族。没装的族返回 nil。可从任意线程调用。
NSFontDescriptor *MSIMEInstalledFontFamilyDescriptorWithWeight(NSString *family, NSFontWeight weight);
