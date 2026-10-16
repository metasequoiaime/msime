#import <AppKit/AppKit.h>
#import "../../src/candidate/CandidatePlacement.h"

#include <cassert>
#include <cmath>
#include <initializer_list>

// Where the candidate window goes, and when the caret it is told about is worth using at all.
//
// Both decisions are made here and nowhere else, and both fail in ways that are obvious to a user
// and invisible to every other test: a window a few points off screen, or one parked at the origin
// of the main display while the user types in the corner of a second one.
int main()
{
    @autoreleasepool
    {
        // A 1600x1000 display whose visible area starts below the menu bar, and a second one to the
        // left of it with negative coordinates - the ordinary two-display arrangement on this
        // platform, and the one that catches an implementation reaching for absolute zero.
        const NSRect screen = NSMakeRect(0, 0, 1600, 1000);
        const NSRect left = NSMakeRect(-1440, 200, 1440, 900);
        const NSSize panel = NSMakeSize(320, 120);

        // The ordinary case: under the caret, four points clear of it, left edge aligned.
        const NSRect caret = NSMakeRect(400, 600, 2, 18);
        const NSPoint under = MSIMECandidateOrigin(caret, panel, screen);
        assert(under.x == 400);
        assert(under.y == 600 - 120 - 4);

        // Near the bottom of the display there is no room underneath, so it flips above the caret -
        // above the whole caret, not above its baseline, or it would cover the line being typed.
        const NSRect low = NSMakeRect(400, 40, 2, 18);
        const NSPoint flipped = MSIMECandidateOrigin(low, panel, screen);
        assert(flipped.y == NSMaxY(low) + 4);
        assert(flipped.y >= NSMinY(screen));

        // Against the right edge the window stops at the edge rather than hanging off it.
        const NSRect right = NSMakeRect(1500, 600, 2, 18);
        assert(MSIMECandidateOrigin(right, panel, screen).x == 1600 - 320);

        // Against the left edge of a display that starts at a negative x, the clamp is that
        // display's edge. Clamping to zero would drag the window onto the main display.
        const NSRect far = NSMakeRect(-1430, 800, 2, 18);
        const NSPoint negative = MSIMECandidateOrigin(far, panel, left);
        assert(negative.x == -1430);
        const NSRect beyond = NSMakeRect(-1500, 800, 2, 18);
        assert(MSIMECandidateOrigin(beyond, panel, left).x == NSMinX(left));

        // A window wider than the display it is on: the inner clamp keeps the left edge on the
        // display instead of producing a right-hand clamp that is further left than the left one.
        const NSSize wide = NSMakeSize(2000, 120);
        assert(MSIMECandidateOrigin(caret, wide, screen).x == NSMinX(screen));
        // Same for a window taller than the display.
        const NSSize tall = NSMakeSize(320, 2000);
        assert(MSIMECandidateOrigin(caret, tall, screen).y == NSMinY(screen));

        // A caret at the very top still leaves the window inside the visible area after the flip.
        const NSRect top = NSMakeRect(400, 995, 2, 18);
        const NSPoint high = MSIMECandidateOrigin(top, panel, screen);
        assert(high.y >= NSMinY(screen) && high.y + panel.height <= NSMaxY(screen));

        // Flip hysteresis, as Windows' AdjustCandidateWindowPosition does it: the side of the caret is chosen with the tallest vertical list shown since the panel appeared, the position with the current height. Near the bottom a full 300pt list does not fit below, so a 100pt list that has shrunk from it stays above the caret, flush against the line rather than 300pt up.
        const NSRect mid = NSMakeRect(400, 250, 2, 18);
        const NSSize shortList = NSMakeSize(320, 100);
        const NSSize fullList = NSMakeSize(320, 300);
        assert(MSIMECandidateOrigin(mid, shortList, screen).y == 250 - 100 - 4);
        assert(MSIMECandidateOrigin(mid, fullList, screen).y == NSMaxY(mid) + 4);
        assert(MSIMECandidateOrigin(mid, shortList, screen, 300).y == NSMaxY(mid) + 4);
        // A remembered height smaller than the current one never overrides it, and the three-argument form is the no-memory case.
        assert(MSIMECandidateOrigin(mid, shortList, screen, 50).y == 250 - 100 - 4);
        assert(NSEqualPoints(MSIMECandidateOrigin(mid, fullList, screen, 0), MSIMECandidateOrigin(mid, fullList, screen)));
        // With room below for the tallest list too, the memory changes nothing.
        assert(MSIMECandidateOrigin(caret, shortList, screen, 300).y == 600 - 100 - 4);

        // 皮肤装饰：窗口顶上有一条 104pt 的透明带，卡片 120pt，窗口共 224pt。放在光标下方时卡片顶边在光标下方 4pt，透明带朝上伸过光标这一行，卡片和光标之间不再隔着整条带。
        const CGFloat band = 104;
        const NSSize decorated = NSMakeSize(320, 120 + band);
        const NSPoint card = MSIMECandidateOrigin(caret, decorated, screen, 0, band);
        assert(card.x == 400);
        assert(card.y == 600 - 120 - 4);
        assert(card.y + decorated.height - band == NSMinY(caret) - 4);
        assert(card.y + decorated.height > NSMaxY(caret));
        // 没有装饰带时和以前逐点相同；负的带高当 0。
        for (const NSRect probe : {caret, low, right, far, top, mid})
            for (const NSRect display : {screen, left})
                for (const CGFloat memory : {0.0, 300.0}) {
                    assert(NSEqualPoints(MSIMECandidateOrigin(probe, panel, display, memory, 0), MSIMECandidateOrigin(probe, panel, display, memory)));
                    assert(NSEqualPoints(MSIMECandidateOrigin(probe, panel, display, memory, -20), MSIMECandidateOrigin(probe, panel, display, memory)));
                }
        // 光标下方连卡片都放不下才翻到上方。翻上去以后透明带在最上面，窗口底边就是卡片底边，照旧贴着光标上方 4pt。
        const NSPoint lifted = MSIMECandidateOrigin(low, decorated, screen, 0, band);
        assert(lifted.y == NSMaxY(low) + 4);
        // 翻不翻只看卡片：光标离屏幕下边 150pt，放得下 120pt 的卡片，放不下 224pt 的整个窗口。以前按整个窗口判断会翻到上方，现在留在下方。
        const NSRect lowish = NSMakeRect(400, 150, 2, 18);
        assert(MSIMECandidateOrigin(lowish, decorated, screen, 0, 0).y == NSMaxY(lowish) + 4);
        assert(MSIMECandidateOrigin(lowish, decorated, screen, 0, band).y == 150 - 120 - 4);
        // 光标贴近屏幕上边、透明带伸出上边时，整个窗口夹回可见区域：装饰图不被裁掉，卡片和光标之间留出的只是伸出去的那一段，比整条带小。
        const NSRect nearTop = NSMakeRect(400, 960, 2, 18);
        const NSPoint pressed = MSIMECandidateOrigin(nearTop, decorated, screen, 0, band);
        assert(pressed.y == NSMaxY(screen) - decorated.height);
        const CGFloat gap = NSMinY(nearTop) - (pressed.y + decorated.height - band);
        assert(gap > 4 && gap < band + 4);
        assert(gap == 4 + (NSMinY(nearTop) - 4 + band - NSMaxY(screen)));
        // 左右边缘的夹取和装饰带无关。
        assert(MSIMECandidateOrigin(right, decorated, screen, 0, band).x == 1600 - 320);
        assert(MSIMECandidateOrigin(beyond, decorated, left, 0, band).x == NSMinX(left));
        // 屏幕下边：放不下卡片时翻到上方，翻上去也在可见区域里。
        const NSRect bottom = NSMakeRect(400, 2, 2, 18);
        const NSPoint lowest = MSIMECandidateOrigin(bottom, decorated, screen, 0, band);
        assert(lowest.y == NSMaxY(bottom) + 4 && lowest.y >= NSMinY(screen) && lowest.y + decorated.height <= NSMaxY(screen));
        // 竖排的翻转记忆同样只按卡片算：记下的最高窗口 404pt（卡片 300pt）放不下，100pt 的卡片随它留在上方；没有记忆时 100pt 的卡片放在下方。
        const NSSize shortDecorated = NSMakeSize(320, 100 + band);
        const CGFloat tallestDecorated = 300 + band;
        assert(MSIMECandidateOrigin(mid, shortDecorated, screen, tallestDecorated, band).y == NSMaxY(mid) + 4);
        assert(MSIMECandidateOrigin(mid, shortDecorated, screen, 0, band).y == 250 - 100 - 4);
        // 卡片 300pt 时光标下方放不下，翻到上方；记下的高度含装饰带，按卡片算的结果是 250 - 300 - 4 < 0。
        assert(MSIMECandidateOrigin(mid, NSMakeSize(320, tallestDecorated), screen, tallestDecorated, band).y == NSMaxY(mid) + 4);
        // 记忆里的窗口 304pt（卡片 200pt）在光标下方放得下：带进记忆也留在下方，不因为整个窗口高过光标就翻上去。
        assert(MSIMECandidateOrigin(mid, shortDecorated, screen, 200 + band, band).y == 250 - 100 - 4);

        // The memory: vertical lists raise it while the panel stays up, a hidden panel starts over, and horizontal rows neither raise it nor are placed by it.
        CGFloat tallest = MSIMETallestCandidateHeight(0, 100, true, false);
        assert(tallest == 100);
        tallest = MSIMETallestCandidateHeight(tallest, 300, true, true);
        assert(tallest == 300);
        tallest = MSIMETallestCandidateHeight(tallest, 100, true, true);
        assert(tallest == 300);
        assert(MSIMETallestCandidateHeight(tallest, 100, true, false) == 100);
        assert(MSIMETallestCandidateHeight(tallest, 40, false, true) == 300);
        assert(MSIMETallestCandidateHeight(tallest, 40, false, false) == 0);

        // What counts as a caret worth positioning against. A zero-height rect is what a client
        // that does not implement the caret query returns, and NaN is what an arithmetic failure
        // upstream produces; both would otherwise park the window somewhere arbitrary.
        assert(MSIMEValidCaret(caret));
        assert(MSIMEValidCaret(far));
        assert(!MSIMEValidCaret(NSMakeRect(400, 600, 2, 0)));
        assert(!MSIMEValidCaret(NSMakeRect(400, 600, 2, -18)));
        assert(!MSIMEValidCaret(NSMakeRect(NAN, 600, 2, 18)));
        assert(!MSIMEValidCaret(NSMakeRect(400, NAN, 2, 18)));
        assert(!MSIMEValidCaret(NSMakeRect(400, 600, NAN, 18)));
        assert(!MSIMEValidCaret(NSMakeRect(400, 600, 2, NAN)));
        assert(!MSIMEValidCaret(NSMakeRect(INFINITY, 600, 2, 18)));
        // A zero-width caret is ordinary - an insertion point has no width.
        assert(MSIMEValidCaret(NSMakeRect(400, 600, 0, 18)));

        // 选屏幕：主屏、左边坐标为负的副屏，再加一块接在主屏右边、比主屏高的屏幕，顺序同 NSScreen.screens（主屏在前）。
        const NSRect rightScreen = NSMakeRect(1600, 0, 1920, 1080);
        NSArray<NSValue *> *frames = @[[NSValue valueWithRect:screen], [NSValue valueWithRect:left], [NSValue valueWithRect:rightScreen]];
        assert(MSIMECandidateScreenIndex(NSMakePoint(400, 600), frames) == 0);
        assert(MSIMECandidateScreenIndex(NSMakePoint(-700, 800), frames) == 1);
        assert(MSIMECandidateScreenIndex(NSMakePoint(2000, 1050), frames) == 2);
        // 两块屏幕共用的边照旧归右边、上边那块：x = 1600 是主屏的右边，也是右屏的左边。
        assert(MSIMECandidateScreenIndex(NSMakePoint(1600, 500), frames) == 2);
        assert(MSIMECandidateScreenIndex(NSMakePoint(-1440, 200), frames) == 1);
        // 贴着最右那块屏幕的右边、或者某块屏幕的上边，NSPointInRect 哪块都不含；这时要留在这块屏幕上，不能退回主屏。
        assert(MSIMECandidateScreenIndex(NSMakePoint(3520, 500), frames) == 2);
        assert(MSIMECandidateScreenIndex(NSMakePoint(2000, 1080), frames) == 2);
        assert(MSIMECandidateScreenIndex(NSMakePoint(3520, 1080), frames) == 2);
        // 坐标为负的副屏的上边：x 在副屏里，主屏离它还有一段，取副屏。
        assert(MSIMECandidateScreenIndex(NSMakePoint(-1, 1100), frames) == 1);
        assert(MSIMECandidateScreenIndex(NSMakePoint(-1440, 1100), frames) == 1);
        // 副屏的右上角同时在主屏的左边上，两块离它都是 0：取排在前面的主屏，结果不随别的东西变。
        assert(MSIMECandidateScreenIndex(NSMakePoint(0, 1000), frames) == 0);
        // 落在屏幕之间的空当里（副屏下方、主屏左边）取最近的那块。
        assert(MSIMECandidateScreenIndex(NSMakePoint(-700, 100), frames) == 1);
        assert(MSIMECandidateScreenIndex(NSMakePoint(-20, 100), frames) == 0);
        assert(MSIMECandidateScreenIndex(NSMakePoint(4000, 500), frames) == 2);
        // 含住锚点的屏幕不看顺序；没有屏幕时交给调用方。
        NSArray<NSValue *> *reversed = frames.reverseObjectEnumerator.allObjects;
        assert(MSIMECandidateScreenIndex(NSMakePoint(-700, 800), reversed) == 1);
        assert(MSIMECandidateScreenIndex(NSMakePoint(3520, 500), reversed) == 0);
        assert(MSIMECandidateScreenIndex(NSMakePoint(400, 600), @[]) == NSNotFound);
    }
    return 0;
}
