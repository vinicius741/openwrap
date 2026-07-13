#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef void (*OpenWrapNEEventCallback)(const char *session_id,
                                        int32_t event,
                                        const char *message);

bool openwrap_ne_start(const char *session_id,
                       const char *provider_bundle_id,
                       const uint8_t *payload,
                       size_t payload_len,
                       OpenWrapNEEventCallback callback);

bool openwrap_ne_stop(const char *session_id);
