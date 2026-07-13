#import "OpenWrapNetworkExtensionBridge.h"

#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>
#import <SystemExtensions/SystemExtensions.h>

enum OpenWrapNEEvent {
    OpenWrapNEEventLog = 0,
    OpenWrapNEEventConnecting = 1,
    OpenWrapNEEventConnected = 2,
    OpenWrapNEEventDisconnected = 3,
    OpenWrapNEEventError = 4,
    OpenWrapNEEventNeedsApproval = 5,
};

@interface OpenWrapNetworkExtensionBridge : NSObject <OSSystemExtensionRequestDelegate>
@property(nonatomic, copy) NSString *sessionID;
@property(nonatomic, copy) NSString *providerBundleID;
@property(nonatomic, strong) NSData *payload;
@property(nonatomic, assign) OpenWrapNEEventCallback callback;
@property(nonatomic, strong) NETunnelProviderManager *manager;
@property(nonatomic, assign) BOOL startRequested;
@property(nonatomic, assign) BOOL terminalEventSent;
@property(nonatomic, assign) BOOL cancelled;
@property(nonatomic, assign) BOOL userStopRequested;
@property(nonatomic, copy) NSString *lastProviderError;
@property(nonatomic, strong) dispatch_source_t heartbeatTimer;
@end

@implementation OpenWrapNetworkExtensionBridge

+ (instancetype)shared {
    static OpenWrapNetworkExtensionBridge *bridge;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
      bridge = [[self alloc] init];
    });
    return bridge;
}

- (void)emit:(enum OpenWrapNEEvent)event message:(NSString *)message {
    OpenWrapNEEventCallback callback = self.callback;
    NSString *sessionID = self.sessionID;
    if (callback == NULL || sessionID.length == 0) {
        return;
    }
    callback(sessionID.UTF8String, event, (message ?: @"").UTF8String);
}

- (void)completePreviousSessionIfNeeded {
    if (self.sessionID.length == 0 || self.terminalEventSent) {
        return;
    }
    self.terminalEventSent = YES;
    [self stopHeartbeat];
    if (self.manager != nil && self.startRequested) {
        [self.manager.connection stopVPNTunnel];
    }
    [self emit:OpenWrapNEEventError
       message:@"Superseded by a new Packet Tunnel connection request"];
    self.callback = NULL;
    self.sessionID = nil;
    self.payload = nil;
    self.manager = nil;
    self.startRequested = NO;
    self.cancelled = NO;
    self.userStopRequested = NO;
    self.lastProviderError = nil;
}

- (void)startSession:(NSString *)sessionID
      providerBundleID:(NSString *)providerBundleID
                payload:(NSData *)payload
               callback:(OpenWrapNEEventCallback)callback {
    dispatch_async(dispatch_get_main_queue(), ^{
      // Reject overlapping activation: finish any in-flight session first so its
      // registry entry is not stranded under a reused singleton.
      if (self.sessionID.length > 0 &&
          (!self.terminalEventSent || self.startRequested) &&
          ![self.sessionID isEqualToString:sessionID]) {
          [self completePreviousSessionIfNeeded];
      } else if (self.sessionID.length > 0 &&
                 [self.sessionID isEqualToString:sessionID] &&
                 !self.terminalEventSent) {
          // Same session id already active — ignore duplicate start.
          return;
      }

      self.sessionID = sessionID;
      self.providerBundleID = providerBundleID;
      self.payload = payload;
      self.callback = callback;
      self.startRequested = NO;
      self.terminalEventSent = NO;
      self.cancelled = NO;
      self.userStopRequested = NO;
      self.lastProviderError = nil;
      [self stopHeartbeat];

      [[NSNotificationCenter defaultCenter]
          removeObserver:self
                    name:NEVPNStatusDidChangeNotification
                  object:nil];
      [[NSNotificationCenter defaultCenter]
          addObserver:self
             selector:@selector(vpnStatusDidChange:)
                 name:NEVPNStatusDidChangeNotification
               object:nil];

      OSSystemExtensionRequest *request =
          [OSSystemExtensionRequest activationRequestForExtension:providerBundleID
                                                             queue:dispatch_get_main_queue()];
      request.delegate = self;
      [[OSSystemExtensionManager sharedManager] submitRequest:request];
    });
}

- (void)configureAndStartTunnel {
    if (self.cancelled) {
        return;
    }
    [NETunnelProviderManager
        loadAllFromPreferencesWithCompletionHandler:^(NSArray<NETunnelProviderManager *> *managers,
                                                       NSError *error) {
          if (error != nil) {
              [self fail:[NSString stringWithFormat:@"Could not load VPN preferences: %@",
                                                        error.localizedDescription]];
              return;
          }
          if (self.cancelled) {
              return;
          }

          NETunnelProviderManager *manager = nil;
          for (NETunnelProviderManager *candidate in managers) {
              NETunnelProviderProtocol *protocol =
                  (NETunnelProviderProtocol *)candidate.protocolConfiguration;
              if ([protocol.providerBundleIdentifier isEqualToString:self.providerBundleID]) {
                  manager = candidate;
                  break;
              }
          }
          if (manager == nil) {
              manager = [[NETunnelProviderManager alloc] init];
          }

          NETunnelProviderProtocol *protocol = [[NETunnelProviderProtocol alloc] init];
          protocol.providerBundleIdentifier = self.providerBundleID;
          protocol.serverAddress = @"OpenWrap Packet Tunnel";
          // Profile contents and credentials are deliberately not persisted here.
          // They are supplied as ephemeral start options for this connection only.
          manager.protocolConfiguration = protocol;
          manager.localizedDescription = @"OpenWrap";
          manager.enabled = YES;
          self.manager = manager;

          [manager saveToPreferencesWithCompletionHandler:^(NSError *saveError) {
            if (saveError != nil) {
                [self fail:[NSString stringWithFormat:@"Could not save VPN preferences: %@",
                                                      saveError.localizedDescription]];
                return;
            }
            [manager loadFromPreferencesWithCompletionHandler:^(NSError *loadError) {
              if (loadError != nil) {
                  [self fail:[NSString stringWithFormat:@"Could not reload VPN preferences: %@",
                                                        loadError.localizedDescription]];
                  return;
              }
              if (self.cancelled) {
                  return;
              }
              NETunnelProviderSession *session =
                  (NETunnelProviderSession *)manager.connection;
              NSError *startError = nil;
              self.startRequested = YES;
              NSDictionary<NSString *, id> *options = @{
                  @"openwrapPayload" : self.payload,
                  @"openwrapSessionID" : self.sessionID,
              };
              if (![session startTunnelWithOptions:options andReturnError:&startError]) {
                  [self fail:[NSString stringWithFormat:@"Could not start Packet Tunnel: %@",
                                                        startError.localizedDescription]];
                  return;
              }
              self.payload = nil;
              [self startHeartbeat];
              [self emit:OpenWrapNEEventConnecting message:@"Packet Tunnel start requested"];
            }];
          }];
        }];
}

- (void)vpnStatusDidChange:(NSNotification *)notification {
    if (notification.object != self.manager.connection || !self.startRequested) {
        return;
    }
    switch (self.manager.connection.status) {
    case NEVPNStatusConnecting:
    case NEVPNStatusReasserting:
        [self emit:OpenWrapNEEventConnecting message:@"Packet Tunnel is connecting"];
        [self pollProviderStatus];
        break;
    case NEVPNStatusConnected:
        [self emit:OpenWrapNEEventConnected message:@"Packet Tunnel connected"];
        [self pollProviderStatus];
        break;
    case NEVPNStatusDisconnecting:
        [self emit:OpenWrapNEEventLog message:@"Packet Tunnel is disconnecting"];
        [self pollProviderStatus];
        break;
    case NEVPNStatusDisconnected:
    case NEVPNStatusInvalid:
        [self pollProviderStatus];
        if (!self.terminalEventSent) {
            self.terminalEventSent = YES;
            [self stopHeartbeat];
            if (self.userStopRequested || self.cancelled) {
                [self emit:OpenWrapNEEventDisconnected message:@"Packet Tunnel disconnected"];
            } else if (self.lastProviderError.length > 0) {
                [self emit:OpenWrapNEEventError message:self.lastProviderError];
            } else {
                [self emit:OpenWrapNEEventError
                   message:@"Packet Tunnel disconnected unexpectedly"];
            }
        }
        break;
    }
}

- (void)stopSession:(NSString *)sessionID {
    dispatch_async(dispatch_get_main_queue(), ^{
      if (![self.sessionID isEqualToString:sessionID]) {
          return;
      }
      self.cancelled = YES;
      self.userStopRequested = YES;
      [self stopHeartbeat];
      if (self.manager != nil && self.startRequested) {
          [self.manager.connection stopVPNTunnel];
      } else if (!self.terminalEventSent) {
          self.terminalEventSent = YES;
          [self emit:OpenWrapNEEventDisconnected message:@"Packet Tunnel start cancelled"];
      }
    });
}

- (void)startHeartbeat {
    [self stopHeartbeat];
    self.heartbeatTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER,
                                                 0,
                                                 0,
                                                 dispatch_get_main_queue());
    // Poll frequently while connecting so OpenVPN logs and auth failures reach the host.
    dispatch_source_set_timer(self.heartbeatTimer,
                              dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)),
                              (uint64_t)(1 * NSEC_PER_SEC),
                              (uint64_t)(0.25 * NSEC_PER_SEC));
    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(self.heartbeatTimer, ^{
      typeof(self) self = weakSelf;
      if (self == nil) return;
      [self pollProviderStatus];
    });
    dispatch_resume(self.heartbeatTimer);
}

- (void)pollProviderStatus {
    if (self.cancelled || self.manager == nil) {
        return;
    }
    NETunnelProviderSession *session = (NETunnelProviderSession *)self.manager.connection;
    if (![session isKindOfClass:[NETunnelProviderSession class]]) {
        return;
    }
    NSData *request = [@"status" dataUsingEncoding:NSUTF8StringEncoding];
    NSError *error = nil;
    __weak typeof(self) weakSelf = self;
    [session sendProviderMessage:request
                     returnError:&error
                 responseHandler:^(NSData *responseData) {
                   typeof(self) self = weakSelf;
                   if (self == nil || responseData.length == 0) {
                       return;
                   }
                   [self handleProviderStatusResponse:responseData];
                 }];
    if (error != nil) {
        // Provider may already be gone during teardown; fall back to a simple heartbeat.
        NSData *heartbeat = [@"heartbeat" dataUsingEncoding:NSUTF8StringEncoding];
        NSError *heartbeatError = nil;
        [session sendProviderMessage:heartbeat
                         returnError:&heartbeatError
                     responseHandler:^(NSData *responseData) {
                       (void)responseData;
                     }];
    }
}

- (void)handleProviderStatusResponse:(NSData *)responseData {
    NSError *jsonError = nil;
    id object = [NSJSONSerialization JSONObjectWithData:responseData
                                                options:0
                                                  error:&jsonError];
    if (![object isKindOfClass:[NSDictionary class]]) {
        // Legacy plain "ok" heartbeat response.
        return;
    }
    NSDictionary *payload = (NSDictionary *)object;
    NSArray *logs = [payload[@"logs"] isKindOfClass:[NSArray class]] ? payload[@"logs"] : @[];
    for (id line in logs) {
        if ([line isKindOfClass:[NSString class]] && [(NSString *)line length] > 0) {
            [self emit:OpenWrapNEEventLog message:(NSString *)line];
        }
    }
    id lastError = payload[@"lastError"];
    if ([lastError isKindOfClass:[NSString class]] && [(NSString *)lastError length] > 0) {
        self.lastProviderError = (NSString *)lastError;
    }
}

- (void)stopHeartbeat {
    if (self.heartbeatTimer != nil) {
        dispatch_source_cancel(self.heartbeatTimer);
        self.heartbeatTimer = nil;
    }
}

- (void)fail:(NSString *)message {
    if (self.terminalEventSent) {
        return;
    }
    self.terminalEventSent = YES;
    [self stopHeartbeat];
    self.payload = nil;
    [self emit:OpenWrapNEEventError message:message];
}

#pragma mark - OSSystemExtensionRequestDelegate

- (void)request:(OSSystemExtensionRequest *)request
    didFinishWithResult:(OSSystemExtensionRequestResult)result {
    if (self.cancelled) {
        return;
    }
    [self emit:OpenWrapNEEventLog message:@"OpenWrap system extension is active"];
    [self configureAndStartTunnel];
}

- (void)request:(OSSystemExtensionRequest *)request didFailWithError:(NSError *)error {
    [self fail:[NSString stringWithFormat:@"System extension activation failed: %@",
                                              error.localizedDescription]];
}

- (void)requestNeedsUserApproval:(OSSystemExtensionRequest *)request {
    [self emit:OpenWrapNEEventNeedsApproval
       message:@"Approve OpenWrap in System Settings to continue"];
}

- (OSSystemExtensionReplacementAction)
                  request:(OSSystemExtensionRequest *)request
actionForReplacingExtension:(OSSystemExtensionProperties *)existing
             withExtension:(OSSystemExtensionProperties *)replacement {
    return OSSystemExtensionReplacementActionReplace;
}

@end

bool openwrap_ne_start(const char *session_id,
                       const char *provider_bundle_id,
                       const uint8_t *payload,
                       size_t payload_len,
                       OpenWrapNEEventCallback callback) {
    if (session_id == NULL || provider_bundle_id == NULL || payload == NULL ||
        payload_len == 0 || callback == NULL) {
        return false;
    }
    NSString *sessionID = [NSString stringWithUTF8String:session_id];
    NSString *providerBundleID = [NSString stringWithUTF8String:provider_bundle_id];
    NSData *payloadData = [NSData dataWithBytes:payload length:payload_len];
    if (sessionID == nil || providerBundleID == nil || payloadData == nil) {
        return false;
    }
    [[OpenWrapNetworkExtensionBridge shared] startSession:sessionID
                                          providerBundleID:providerBundleID
                                                    payload:payloadData
                                                   callback:callback];
    return true;
}

bool openwrap_ne_stop(const char *session_id) {
    if (session_id == NULL) {
        return false;
    }
    NSString *sessionID = [NSString stringWithUTF8String:session_id];
    if (sessionID == nil) {
        return false;
    }
    [[OpenWrapNetworkExtensionBridge shared] stopSession:sessionID];
    return true;
}
