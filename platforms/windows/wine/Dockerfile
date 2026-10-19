# Runs the cross-built Windows test executables without a Windows machine.
#
# Wine 10 rather than whatever is oldest: the Rust host library pulls in
# bcryptprimitives.dll, which Wine 8 does not implement, and four suites fail
# to load at all without it. linux/amd64 because the binaries are x86_64 PE;
# on an arm64 host Docker emulates, which is slow but correct.
#
# Xvfb because two suites create real windows. Without a display Wine falls back
# to a null driver, window creation fails, and those suites report an
# environment failure that is really a missing X server - which this image can
# supply, unlike a real monitor.
FROM debian:trixie-slim
# The i386 architecture is what brings WoW64 in. Without wine32 the 64-bit Wine
# has no C:\windows\syswow64\ntdll.dll, and every 32-bit executable fails to
# start - which reads as the whole x86 suite failing rather than as the runner
# being unable to run it at all.
RUN dpkg --add-architecture i386 \
 && apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends wine wine32 wine64 xvfb xauth libgl1:i386 libgl1-mesa-dri:i386 \
 && rm -rf /var/lib/apt/lists/*
# 32 位 Direct2D 也通过 OpenGL 建立 WIC 绘制目标，不能只安装 amd64 的 Mesa。
ENV WINEDEBUG=-all
ENV WINEPREFIX=/wine
# Create the prefix under a display. Without one, prefix creation completes far
# enough to look successful and leaves WIC unregistered: CoCreateInstance for
# CLSID_WICImagingFactory then returns REGDB_E_CLASSNOTREG even though
# windowscodecs.dll is present, and every surface that builds Direct2D factories
# fails before it reaches anything graphical. Waiting for wineserver matters too
# - the registry is still being written when wineboot returns.
RUN xvfb-run -a sh -c 'wineboot --init >/dev/null 2>&1; wineserver -w' \
 && test -d "$WINEPREFIX/drive_c/windows/system32"
