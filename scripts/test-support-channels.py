#!/usr/bin/env python3
"""The QQ group and the Telegram link are the same on every host that lists them.

The shared settings surfaces use one public component for the two strings. Native hosts still carry
their own copies, so a typo in one of them sends that platform's users to a group that does not
exist, and nothing else in this repository would notice, because each copy is correct on its own
terms.

Android is the reason this exists: it shipped with a feedback screen that listed neither channel at
all, so its users' only route was the GitHub issue form. Adding the fifth copy without a check
would have been the wrong trade.

Every host that lists the channels must carry both, byte for byte. A host that lists neither is not
a failure here - that is a product decision about that host's feedback surface, and this check has
nothing to say about it. What it rejects is a host that carries one of them differently.
"""
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]

QQ_GROUP = "829919142"
TELEGRAM = "t.me/msimegroup"

SURFACES = {
    "shared feedback channel component": "packages/ui/src/settings/feedback-channels.tsx",
    "macOS": "platforms/macos/src/core/SupportWindowController.mm",
    "iOS": "platforms/ios/App/Sources/settings/HelpAndFeedbackViews.swift",
    "Android strings": "platforms/android/res/values/strings.xml",
    "Android feedback page": "platforms/android/java/app/msime/android/home/FeedbackPage.java",
}

shared_page = ROOT / "packages/ui/src/settings/pages/feedback-page.tsx"
if not shared_page.exists() or "FeedbackChannels" not in shared_page.read_text():
    missing = ["shared settings page: does not render the shared FeedbackChannels component"]
else:
    missing = []
listing = 0
for host, relative in SURFACES.items():
    path = ROOT / relative
    if not path.exists():
        missing.append(f"{host}: {relative} is gone; this check is pointed at nothing")
        continue
    text = path.read_text()
    has_group = QQ_GROUP in text
    has_telegram = TELEGRAM in text
    if not has_group and not has_telegram:
        missing.append(f"{host}: lists neither channel ({relative})")
        continue
    listing += 1
    if not has_group:
        missing.append(f"{host}: has the Telegram group but not QQ {QQ_GROUP} ({relative})")
    if not has_telegram:
        missing.append(f"{host}: has the QQ group but not {TELEGRAM} ({relative})")

if missing:
    for line in missing:
        print(line)
    sys.exit(
        "the support channels differ between hosts; they are one group and one link, "
        "and a host that names only one of them sends its users to a dead end"
    )

print(f"support channels: QQ {QQ_GROUP} and {TELEGRAM} identical across {listing} surfaces")
