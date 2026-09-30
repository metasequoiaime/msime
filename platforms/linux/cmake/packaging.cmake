# Packaging is explicit and consumes the same installed files as CMake install.
# Never put a prepared user's runtime options into a redistributable package.
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
  message(FATAL_ERROR "MSIME packaging requires a Linux build")
endif()
if(NOT CMAKE_INSTALL_PREFIX STREQUAL "/usr")
  message(FATAL_ERROR "Configure distributable Linux packages with CMAKE_INSTALL_PREFIX=/usr")
endif()
if(MSIME_RUNTIME_OPTIONS_FILE)
  message(FATAL_ERROR "Packaged builds must not include prepared runtime options; leave MSIME_RUNTIME_OPTIONS_FILE empty")
endif()
if(MSIME_LINUX_VOICE)
  message(FATAL_ERROR "Disable MSIME_LINUX_VOICE for packaging; it installs a development test executable")
endif()

# CMakeLists.txt resolves the version before the IBus host is compiled, because the host reports the same version at startup.
set(CPACK_PACKAGE_VERSION "${MSIME_LINUX_VERSION}")
set(CPACK_PACKAGE_NAME "msime-linux")
set(CPACK_PACKAGE_VENDOR "Metasequoia IME")
set(CPACK_PACKAGE_CONTACT "Metasequoia IME <metasequoiaime@gmail.com>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "MSIME Linux IBus host and desktop tools (Fcitx5 addon available)")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/metasequoiaime/msime")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE")
set(CPACK_GENERATOR "TGZ")
set(CPACK_SET_DESTDIR ON)
set(CPACK_PACKAGE_RELOCATABLE FALSE)
set(CPACK_PACKAGE_FILE_NAME "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}-linux-${CMAKE_SYSTEM_PROCESSOR}")
set(CPACK_DEBIAN_FILE_NAME DEB-DEFAULT)
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
     CONTENT "${MSIME_XDG_AUTOSTART_DIR}/msime-linux-clipboard.desktop\n")
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
set(CPACK_RPM_SPEC_MORE_DEFINE "%global __requires_exclude ^lib(msime_host_api|sherpa-onnx-c-api|onnxruntime)\\\\.so.*$
%global __provides_exclude_from ^${CMAKE_INSTALL_FULL_LIBDIR}/msime-client/.*$")
# Directories the base system owns. An RPM that lists them conflicts with the filesystem package and with the desktop, IBus, Fcitx5 and systemd packages that own them.
list(APPEND CPACK_RPM_EXCLUDE_FROM_AUTO_FILELIST_ADDITION
  /etc/xdg /etc/xdg/autostart
  /usr/libexec /usr/lib/systemd /usr/lib/systemd/user
  /usr/share/applications /usr/share/icons /usr/share/icons/hicolor /usr/share/metainfo /usr/share/licenses
  /usr/share/ibus /usr/share/ibus/component
  /usr/share/fcitx5 /usr/share/fcitx5/addon /usr/share/fcitx5/inputmethod
  "${CMAKE_INSTALL_FULL_LIBDIR}/fcitx5")
# The autostart entry is the package's one file under /etc: the RPM counterpart of the Debian conffile above.
set(CPACK_RPM_USER_FILELIST "%config(noreplace) ${MSIME_XDG_AUTOSTART_DIR}/msime-linux-clipboard.desktop")
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
        DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/msime-client" RENAME README.md)
include(CPack)
