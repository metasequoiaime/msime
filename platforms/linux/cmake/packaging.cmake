# Packaging is explicit and consumes the same installed files as CMake install.
# Never put a prepared user's runtime options into a redistributable package.
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
  message(FATAL_ERROR "MSIME packaging requires a Linux build")
endif()
# full 的包装在 /usr 下；其他版本的包装在版本表给的前缀（/opt/msime-linux-<id>）下，两者都由 cmake/Edition.cmake 读出。
if(NOT CMAKE_INSTALL_PREFIX STREQUAL MSIME_EDITION_INSTALL_PREFIX)
  message(FATAL_ERROR "Configure distributable Linux packages of edition ${MSIME_EDITION} with CMAKE_INSTALL_PREFIX=${MSIME_EDITION_INSTALL_PREFIX}")
endif()
if(MSIME_RUNTIME_OPTIONS_FILE)
  message(FATAL_ERROR "Packaged builds must not include prepared runtime options; leave MSIME_RUNTIME_OPTIONS_FILE empty")
endif()
if(MSIME_LINUX_VOICE)
  message(FATAL_ERROR "Disable MSIME_LINUX_VOICE for packaging; it installs a development test executable")
endif()

# CMakeLists.txt resolves the version before the IBus host is compiled, because the host reports the same version at startup.
set(CPACK_PACKAGE_VERSION "${MSIME_LINUX_VERSION}")
# 包名取自版本表（full 是 msime-linux）：各版本是互不替换的独立包，可以同时安装。
set(CPACK_PACKAGE_NAME "${MSIME_EDITION_PACKAGE}")
set(CPACK_PACKAGE_VENDOR "Metasequoia IME")
set(CPACK_PACKAGE_CONTACT "Metasequoia IME <metasequoiaime@gmail.com>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "${MSIME_EDITION_DISPLAY_NAME_EN} Linux IBus host and desktop tools (Fcitx5 addon available)")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/metasequoiaime/msime")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE")
set(CPACK_GENERATOR "TGZ")
set(CPACK_SET_DESTDIR ON)
set(CPACK_PACKAGE_RELOCATABLE FALSE)
# Strip what CMake builds and installs with install(TARGETS): the native hosts, the Fcitx5 addon and msime-voice-local. The Rust binaries come in through install(FILES/PROGRAMS), which this does not reach, so package-container.sh builds them stripped; the sherpa-onnx and ONNX Runtime libraries are upstream's prebuilt files and stay as they come, as debian/rules keeps them. Only CPack reads this: debian/rules, rpm/msime.spec and the PKGBUILD install with cmake --install, so dh_strip and find-debuginfo still get the debug info they split out.
set(CPACK_STRIP_FILES TRUE)
set(CPACK_PACKAGE_FILE_NAME "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}-linux-${CMAKE_SYSTEM_PROCESSOR}")
set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
# xz rather than gzip for data.tar. The CMake 3.25 in Dockerfile.package has no CPACK_DEBIAN_COMPRESSION_LEVEL and uses xz preset 6, so package-container.sh repacks each .deb at -9 with dpkg-deb. Every dpkg the package supports reads xz members. The .tar.gz stays gzip: the in-app update check falls back to it by name.
set(CPACK_DEBIAN_COMPRESSION_TYPE "xz")
set(CPACK_DEBIAN_PACKAGE_SECTION "utils")
set(CPACK_DEBIAN_PACKAGE_PRIORITY "optional")
# procps provides the pgrep msime-linux-setup uses to see whether the input method is running before it switches dictionaries; a system without it fails every dictionary switch. Debian marks procps important rather than required, so a minimal install can lack it.
set(CPACK_DEBIAN_PACKAGE_DEPENDS "ibus (>= 1.5.20), python3 (>= 3.9), procps")
if(MSIME_ENABLE_FCITX5)
  string(APPEND CPACK_DEBIAN_PACKAGE_DEPENDS ", fcitx5 (>= 5.0.20)")
endif()
# Voice runtime: Doubao streaming needs the websockets sync client from 15.0 on, recording needs one of parec, pw-cat or arecord. Recommends rather than Depends, because the voice service starts without them and only the requests that need them fail.
set(CPACK_DEBIAN_PACKAGE_RECOMMENDS "python3-websockets (>= 15), pulseaudio-utils | pipewire-bin | alsa-utils")
set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
get_filename_component(MSIME_HOST_LIBRARY_DIR "${MSIME_HOST_LIBRARY}" DIRECTORY)
# libsherpa-onnx-c-api.so needs libonnxruntime.so, which ships beside it in the same private directory rather than coming from a Debian package.
set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS_PRIVATE_DIRS "${MSIME_HOST_LIBRARY_DIR};${MSIME_VOICE_RUNTIME_DIR}")
# prerm stops and disables the user units of logged-in users on removal and postinst restarts running services after an upgrade; CMakeLists.txt configures both from the unit list the CMake uninstall uses.
# The clipboard XDG autostart entry is the package's one file under /etc (a /usr prefix puts MSIME_XDG_AUTOSTART_DIR there), and Debian policy requires /etc files to be conffiles so an administrator who edits or deletes it keeps that change across upgrades. CPack's DEB generator marks nothing by itself; the list travels as a control file like the maintainer scripts.
file(CONFIGURE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/debian/conffiles"
     CONTENT "${MSIME_XDG_AUTOSTART_DIR}/${MSIME_EDITION_PACKAGE}-clipboard.desktop\n")
set(CPACK_DEBIAN_PACKAGE_CONTROL_EXTRA "${CMAKE_CURRENT_BINARY_DIR}/debian/prerm;${CMAKE_CURRENT_BINARY_DIR}/debian/postinst;${CMAKE_CURRENT_BINARY_DIR}/debian/conffiles")
set(CPACK_DEBIAN_PACKAGE_CONTROL_STRICT_PERMISSION ON)

# RPM for Fedora and other DNF systems. package-container.sh builds it in a Fedora container, never by converting the .deb: rpmbuild derives Requires from the libraries the binaries link, and a build on Debian records Debian's sonames and symbol versions (libcurl's CURL_OPENSSL_4, boost 1.83) that no Fedora package provides, which is what made the MSIME-Linux 0.9.1 rpm uninstallable (#2095).
set(CPACK_RPM_FILE_NAME RPM-DEFAULT)
set(CPACK_RPM_PACKAGE_LICENSE "GPL-3.0-only")
set(CPACK_RPM_PACKAGE_GROUP "System Environment/Libraries")
set(CPACK_RPM_PACKAGE_URL "${CPACK_PACKAGE_HOMEPAGE_URL}")
set(CPACK_RPM_PACKAGE_REQUIRES "ibus >= 1.5.20, python3 >= 3.9, procps-ng")
if(MSIME_ENABLE_FCITX5)
  string(APPEND CPACK_RPM_PACKAGE_REQUIRES ", fcitx5 >= 5.0.20")
endif()
# The same voice runtime as the .deb Recommends, in Fedora's package names; a rich dependency expresses the alternatives.
set(CPACK_RPM_PACKAGE_RECOMMENDS "python3-websockets >= 15, (pulseaudio-utils or pipewire-utils or alsa-utils)")
# The host library and the sherpa-onnx runtime ship in the package's private directory, as CPACK_DEBIAN_PACKAGE_SHLIBDEPS_PRIVATE_DIRS says for the .deb: nothing may require them from the system, and the package must not advertise them as system libraries either.
# The payload is xz at level 9, as for the .deb, instead of rpmbuild's default zstd; CPACK_RPM_COMPRESSION_TYPE xz would only give level 7. Without a T in the payload string rpm compresses on one thread, so the 674 MiB xz -9 needs is not multiplied by the core count.
set(CPACK_RPM_SPEC_MORE_DEFINE "%global __requires_exclude ^lib(${MSIME_HOST_LIBRARY_STEM}|sherpa-onnx-c-api|onnxruntime)\\\\.so.*$
%global __provides_exclude_from ^${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}/.*$
%define _binary_payload w9.xzdio")
# Directories the base system owns. An RPM that lists them conflicts with the filesystem package and with the desktop, IBus, Fcitx5 and systemd packages that own them.
list(APPEND CPACK_RPM_EXCLUDE_FROM_AUTO_FILELIST_ADDITION
  /etc/xdg /etc/xdg/autostart
  /usr/libexec /usr/lib/systemd /usr/lib/systemd/user
  /usr/share/applications /usr/share/icons /usr/share/icons/hicolor /usr/share/metainfo /usr/share/licenses
  /usr/share/ibus /usr/share/ibus/component
  /usr/share/fcitx5 /usr/share/fcitx5/addon /usr/share/fcitx5/inputmethod
  "${CMAKE_INSTALL_FULL_LIBDIR}/fcitx5")
# 装在 /opt 下的版本：/opt 归 filesystem 包，插件目录取自 Fcitx5Core.pc（fcitx5/CMakeLists.txt），也归 fcitx5。
if(NOT MSIME_EDITION_IS_FULL)
  list(APPEND CPACK_RPM_EXCLUDE_FROM_AUTO_FILELIST_ADDITION /opt /usr/bin)
  if(MSIME_FCITX5_ADDON_DIR AND IS_ABSOLUTE "${MSIME_FCITX5_ADDON_DIR}")
    list(APPEND CPACK_RPM_EXCLUDE_FROM_AUTO_FILELIST_ADDITION "${MSIME_FCITX5_ADDON_DIR}")
  endif()
  # The Fcitx5 addon has to sit in the system fcitx5 addon directory while the host library stays in this edition's private directory under /opt, so fcitx5/CMakeLists.txt gives the addon an absolute RUNPATH to that directory on purpose; no $ORIGIN path survives the two moving independently. Fedora's check-rpaths (run from %__os_install_post) rejects any absolute RPATH outside the standard library directories as 0x0002 "invalid" and fails %install, so for this package only that one bit is allowed. The check itself still runs, and every other RPATH problem (empty, relative, '..' or $ORIGIN out of order) still fails the build. The full edition installs under /usr with $ORIGIN-relative RUNPATHs and keeps the default check.
  if(MSIME_ENABLE_FCITX5)
    string(APPEND CPACK_RPM_SPEC_MORE_DEFINE "
%global __brp_check_rpaths QA_RPATHS=0x0002 %{_rpmconfigdir}/check-rpaths")
  endif()
endif()
# The autostart entry is the package's one file under /etc: the RPM counterpart of the Debian conffile above.
set(CPACK_RPM_USER_FILELIST "%config(noreplace) ${MSIME_XDG_AUTOSTART_DIR}/${MSIME_EDITION_PACKAGE}-clipboard.desktop")
# The maintainer scripts are the Debian ones, which dispatch on dpkg's arguments. RPM passes the number of installed instances instead (%post: 1 on install, 2 or more on upgrade; %preun: 0 on removal, 1 or more on upgrade), so each script is prefixed with the translation to the dpkg call it corresponds to.
file(READ "${CMAKE_CURRENT_BINARY_DIR}/debian/postinst" MSIME_DEB_POSTINST)
file(READ "${CMAKE_CURRENT_BINARY_DIR}/debian/prerm" MSIME_DEB_PRERM)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/rpm/post"
     "if [ \"$1\" -ge 2 ]; then set -- configure upgrade; else set -- configure; fi\n${MSIME_DEB_POSTINST}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/rpm/preun"
     "if [ \"$1\" = 0 ]; then set -- remove; else set -- upgrade; fi\n${MSIME_DEB_PRERM}")
set(CPACK_RPM_POST_INSTALL_SCRIPT_FILE "${CMAKE_CURRENT_BINARY_DIR}/rpm/post")
set(CPACK_RPM_PRE_UNINSTALL_SCRIPT_FILE "${CMAKE_CURRENT_BINARY_DIR}/rpm/preun")

# The license (as copyright) and the third-party notices are installed by CMakeLists.txt for every install; configuration already failed there if any of them was missing.
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/README.md"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/${MSIME_CLIENT_DIRECTORY}" RENAME README.md)
include(CPack)
