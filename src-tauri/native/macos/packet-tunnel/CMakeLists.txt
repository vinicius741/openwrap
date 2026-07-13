cmake_minimum_required(VERSION 3.24)
project(OpenWrapPacketTunnel LANGUAGES C CXX OBJC OBJCXX)

include(FetchContent)
set(FETCHCONTENT_UPDATES_DISCONNECTED ON)
if(POLICY CMP0169)
  cmake_policy(SET CMP0169 OLD)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_OBJCXX_STANDARD 20)
set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING "" FORCE)

set(OPENWRAP_VERSION "0.1.0" CACHE STRING "Bundle version")
set(OPENWRAP_BUILD_NUMBER "1" CACHE STRING "Bundle build number")
set(OPENWRAP_OUTPUT_BUNDLE
    "${CMAKE_BINARY_DIR}/OpenWrapPacketTunnel.systemextension"
    CACHE PATH "Output system extension bundle")

FetchContent_Declare(
  openvpn3
  GIT_REPOSITORY https://github.com/OpenVPN/openvpn3.git
  GIT_TAG 3ae0f347b2033e1a71a2f11a5a01c19748ac2d86
  GIT_SHALLOW FALSE)
FetchContent_Declare(
  asio
  GIT_REPOSITORY https://github.com/chriskohlhoff/asio.git
  GIT_TAG asio-1-30-2
  GIT_SHALLOW TRUE)
FetchContent_Declare(
  fmt
  GIT_REPOSITORY https://github.com/fmtlib/fmt.git
  GIT_TAG 12.2.0
  GIT_SHALLOW TRUE)
FetchContent_Declare(
  lz4
  GIT_REPOSITORY https://github.com/lz4/lz4.git
  GIT_TAG v1.10.0
  GIT_SHALLOW TRUE)
FetchContent_Declare(
  mbedtls
  GIT_REPOSITORY https://github.com/Mbed-TLS/mbedtls.git
  GIT_TAG mbedtls-3.6.4
  GIT_SHALLOW TRUE)

FetchContent_GetProperties(openvpn3)
if(NOT openvpn3_POPULATED)
  FetchContent_Populate(openvpn3)
endif()
FetchContent_GetProperties(asio)
if(NOT asio_POPULATED)
  FetchContent_Populate(asio)
endif()
FetchContent_GetProperties(lz4)
if(NOT lz4_POPULATED)
  FetchContent_Populate(lz4)
endif()

set(FMT_TEST OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(fmt)
set(ENABLE_PROGRAMS OFF CACHE BOOL "" FORCE)
set(ENABLE_TESTING OFF CACHE BOOL "" FORCE)
set(MBEDTLS_FATAL_WARNINGS OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(mbedtls)

file(MAKE_DIRECTORY "${OPENWRAP_OUTPUT_BUNDLE}/Contents/MacOS")
file(MAKE_DIRECTORY "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty")
configure_file(
  "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Info.plist"
  @ONLY)
configure_file(
  "${openvpn3_SOURCE_DIR}/LICENSE.md"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty/OpenVPN3-LICENSE.md"
  COPYONLY)
configure_file(
  "${asio_SOURCE_DIR}/asio/LICENSE_1_0.txt"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty/Asio-LICENSE.txt"
  COPYONLY)
configure_file(
  "${fmt_SOURCE_DIR}/LICENSE"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty/fmt-LICENSE.txt"
  COPYONLY)
configure_file(
  "${lz4_SOURCE_DIR}/LICENSE"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty/LZ4-LICENSE.txt"
  COPYONLY)
configure_file(
  "${mbedtls_SOURCE_DIR}/LICENSE"
  "${OPENWRAP_OUTPUT_BUNDLE}/Contents/Resources/ThirdParty/MbedTLS-LICENSE.txt"
  COPYONLY)

add_executable(OpenWrapPacketTunnel
  main.m
  OpenWrapPacketTunnelProvider.mm
  "${openvpn3_SOURCE_DIR}/client/ovpncli.cpp"
  "${openvpn3_SOURCE_DIR}/openvpn/crypto/data_epoch.cpp"
  "${lz4_SOURCE_DIR}/lib/lz4.c"
  "${lz4_SOURCE_DIR}/lib/lz4frame.c"
  "${lz4_SOURCE_DIR}/lib/lz4hc.c"
  "${lz4_SOURCE_DIR}/lib/xxhash.c")

target_include_directories(OpenWrapPacketTunnel PRIVATE
  "${openvpn3_SOURCE_DIR}"
  "${asio_SOURCE_DIR}/asio/include"
  "${lz4_SOURCE_DIR}/lib")
target_compile_definitions(OpenWrapPacketTunnel PRIVATE
  ASIO_STANDALONE
  USE_ASIO
  USE_MBEDTLS
  HAVE_LZ4
  USE_TUN_BUILDER)
target_compile_options(OpenWrapPacketTunnel PRIVATE
  "$<$<COMPILE_LANGUAGE:OBJC,OBJCXX>:-fobjc-arc>"
  "$<$<COMPILE_LANGUAGE:CXX,OBJCXX>:-Wno-macro-redefined>")

find_library(FOUNDATION_FRAMEWORK Foundation REQUIRED)
find_library(NETWORK_EXTENSION_FRAMEWORK NetworkExtension REQUIRED)
find_library(CORE_FOUNDATION_FRAMEWORK CoreFoundation REQUIRED)
find_library(IOKIT_FRAMEWORK IOKit REQUIRED)
find_library(CORE_SERVICES_FRAMEWORK CoreServices REQUIRED)
find_library(SYSTEM_CONFIGURATION_FRAMEWORK SystemConfiguration REQUIRED)

target_link_libraries(OpenWrapPacketTunnel PRIVATE
  fmt::fmt
  mbedtls
  mbedx509
  mbedcrypto
  "${FOUNDATION_FRAMEWORK}"
  "${NETWORK_EXTENSION_FRAMEWORK}"
  "${CORE_FOUNDATION_FRAMEWORK}"
  "${IOKIT_FRAMEWORK}"
  "${CORE_SERVICES_FRAMEWORK}"
  "${SYSTEM_CONFIGURATION_FRAMEWORK}")

set_target_properties(OpenWrapPacketTunnel PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${OPENWRAP_OUTPUT_BUNDLE}/Contents/MacOS"
  BUILD_RPATH "@executable_path/../Frameworks")
