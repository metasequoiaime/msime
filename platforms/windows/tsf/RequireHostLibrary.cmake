# The adapter links the Host API's C ABI, so the library is required at configure time.
if(NOT MSIME_HOST_LIBRARY OR NOT IS_ABSOLUTE "${MSIME_HOST_LIBRARY}" OR
   NOT EXISTS "${MSIME_HOST_LIBRARY}" OR IS_DIRECTORY "${MSIME_HOST_LIBRARY}")
  message(FATAL_ERROR
    "MSIME_HOST_LIBRARY must name an existing absolute-path Cargo-built host library "
    "or Windows import library matching the TSF target architecture. "
    "Build msime-host-api for that architecture before configuring the DLL.")
endif()
