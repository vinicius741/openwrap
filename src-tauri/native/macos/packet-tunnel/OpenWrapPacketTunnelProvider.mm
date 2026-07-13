#import <Foundation/Foundation.h>
#import <NetworkExtension/NetworkExtension.h>

#include <arpa/inet.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>
#include <string.h>

#include <algorithm>
#include <atomic>
#include <memory>
#include <string>
#include <vector>

#include <client/ovpncli.hpp>

namespace {

struct TunnelAddress {
    std::string address;
    int prefix;
    bool ipv6;
};

struct TunnelRoute {
    std::string address;
    int prefix;
    bool ipv6;
};

static char PacketQueueKey;

static void SecureClear(std::string &value) {
    if (!value.empty()) {
        volatile char *bytes = reinterpret_cast<volatile char *>(value.data());
        for (size_t index = 0; index < value.size(); index++) {
            bytes[index] = 0;
        }
        value.clear();
    }
}

struct StartMaterial {
    std::string config;
    std::string username;
    std::string password;

    void clear() {
        SecureClear(config);
        SecureClear(username);
        SecureClear(password);
    }

    ~StartMaterial() { clear(); }
};

static NSString *NSStringFromStd(const std::string &value) {
    return [[NSString alloc] initWithBytes:value.data()
                                   length:value.size()
                                 encoding:NSUTF8StringEncoding] ?: @"";
}

static NSString *IPv4Mask(int prefix) {
    prefix = std::clamp(prefix, 0, 32);
    uint32_t mask = prefix == 0 ? 0 : UINT32_MAX << (32 - prefix);
    struct in_addr address = {.s_addr = htonl(mask)};
    char buffer[INET_ADDRSTRLEN] = {};
    inet_ntop(AF_INET, &address, buffer, sizeof(buffer));
    return [NSString stringWithUTF8String:buffer];
}

static void AppendUnique(NSMutableArray<NSString *> *values, NSString *value) {
    if (value.length > 0 && ![values containsObject:value]) {
        [values addObject:value];
    }
}

static BOOL IsAssetDirective(NSString *directive) {
    static NSSet<NSString *> *directives;
    static dispatch_once_t onceToken;
    dispatch_once(&onceToken, ^{
      directives = [NSSet setWithArray:@[
          @"ca", @"cert", @"key", @"pkcs12", @"tls-auth", @"tls-crypt", @"pem"
      ]];
    });
    return [directives containsObject:directive.lowercaseString];
}

static NSString *RewriteAssetReference(NSString *config,
                                       NSString *relativePath,
                                       NSString *absolutePath) {
    NSMutableArray<NSString *> *lines =
        [[config componentsSeparatedByString:@"\n"] mutableCopy];
    NSCharacterSet *whitespace = [NSCharacterSet whitespaceCharacterSet];
    for (NSUInteger lineIndex = 0; lineIndex < lines.count; lineIndex++) {
        NSString *line = lines[lineIndex];
        NSUInteger cursor = 0;
        while (cursor < line.length && [whitespace characterIsMember:[line characterAtIndex:cursor]]) {
            cursor++;
        }
        if (cursor >= line.length || [line characterAtIndex:cursor] == '#' ||
            [line characterAtIndex:cursor] == ';') {
            continue;
        }

        NSUInteger directiveStart = cursor;
        while (cursor < line.length &&
               ![whitespace characterIsMember:[line characterAtIndex:cursor]]) {
            cursor++;
        }
        NSString *directive = [line substringWithRange:NSMakeRange(directiveStart,
                                                                    cursor - directiveStart)];
        if (!IsAssetDirective(directive)) {
            continue;
        }
        while (cursor < line.length && [whitespace characterIsMember:[line characterAtIndex:cursor]]) {
            cursor++;
        }
        NSUInteger argumentStart = cursor;
        while (cursor < line.length &&
               ![whitespace characterIsMember:[line characterAtIndex:cursor]]) {
            cursor++;
        }
        if (argumentStart == cursor) {
            continue;
        }
        NSRange argumentRange = NSMakeRange(argumentStart, cursor - argumentStart);
        if ([[line substringWithRange:argumentRange] isEqualToString:relativePath]) {
            lines[lineIndex] = [line stringByReplacingCharactersInRange:argumentRange
                                                              withString:absolutePath];
        }
    }
    return [lines componentsJoinedByString:@"\n"];
}

} // namespace

@class OpenWrapPacketTunnelProvider;

@interface OpenWrapPacketTunnelProvider : NEPacketTunnelProvider
- (int)establishPacketBridgeWithSettings:(NEPacketTunnelNetworkSettings *)settings
                                    error:(NSString **)errorMessage;
- (void)stopPacketBridge;
- (void)openVPNEventName:(NSString *)name
                    info:(NSString *)info
                   error:(BOOL)isError
                   fatal:(BOOL)isFatal;
- (void)openVPNLog:(NSString *)line;
@end

class OpenWrapVPNClient final : public openvpn::ClientAPI::OpenVPNClient {
  public:
    OpenWrapVPNClient(OpenWrapPacketTunnelProvider *provider,
                      NSString *dnsPolicy,
                      NSArray<NSString *> *fallbackDnsIntent)
        : provider_(provider),
          dns_policy_([dnsPolicy copy]),
          fallback_dns_intent_([fallbackDnsIntent copy]) {}

    bool tun_builder_new() override {
        remote_address_.clear();
        addresses_.clear();
        routes_.clear();
        excluded_routes_.clear();
        dns_servers_.clear();
        dns_domains_.clear();
        dns_search_domains_.clear();
        reroute_ipv4_ = false;
        reroute_ipv6_ = false;
        mtu_ = 0;
        return true;
    }

    bool tun_builder_set_layer(int layer) override { return layer == 3; }

    bool tun_builder_set_remote_address(const std::string &address, bool ipv6) override {
        remote_address_ = address;
        return true;
    }

    bool tun_builder_add_address(const std::string &address,
                                 int prefix_length,
                                 const std::string &gateway,
                                 bool ipv6,
                                 bool net30) override {
        addresses_.push_back({address, prefix_length, ipv6});
        return true;
    }

    bool tun_builder_reroute_gw(bool ipv4, bool ipv6, unsigned int flags) override {
        reroute_ipv4_ = ipv4;
        reroute_ipv6_ = ipv6;
        return true;
    }

    bool tun_builder_add_route(const std::string &address,
                               int prefix_length,
                               int metric,
                               bool ipv6) override {
        routes_.push_back({address, prefix_length, ipv6});
        return true;
    }

    bool tun_builder_exclude_route(const std::string &address,
                                   int prefix_length,
                                   int metric,
                                   bool ipv6) override {
        excluded_routes_.push_back({address, prefix_length, ipv6});
        return true;
    }

    bool tun_builder_set_dns_options(const openvpn::DnsOptions &dns) override {
        dns_servers_.clear();
        dns_domains_.clear();
        dns_search_domains_.clear();
        for (const auto &[priority, server] : dns.servers) {
            for (const auto &address : server.addresses) {
                dns_servers_.push_back(address.address);
            }
            for (const auto &domain : server.domains) {
                dns_domains_.push_back(domain.domain);
            }
        }
        for (const auto &domain : dns.search_domains) {
            dns_search_domains_.push_back(domain.domain);
        }
        return true;
    }

    bool tun_builder_set_mtu(int mtu) override {
        mtu_ = mtu;
        return true;
    }

    bool tun_builder_set_session_name(const std::string &name) override { return true; }

    bool tun_builder_persist() override { return false; }

    void tun_builder_teardown(bool disconnect) override {
        OpenWrapPacketTunnelProvider *provider = provider_;
        [provider stopPacketBridge];
    }

    int tun_builder_establish() override {
        OpenWrapPacketTunnelProvider *provider = provider_;
        if (provider == nil || remote_address_.empty()) {
            return -1;
        }

        NEPacketTunnelNetworkSettings *settings =
            [[NEPacketTunnelNetworkSettings alloc]
                initWithTunnelRemoteAddress:NSStringFromStd(remote_address_)];

        NSMutableArray<NSString *> *ipv4Addresses = [NSMutableArray array];
        NSMutableArray<NSString *> *ipv4Masks = [NSMutableArray array];
        NSMutableArray<NSString *> *ipv6Addresses = [NSMutableArray array];
        NSMutableArray<NSNumber *> *ipv6Prefixes = [NSMutableArray array];
        for (const TunnelAddress &address : addresses_) {
            if (address.ipv6) {
                [ipv6Addresses addObject:NSStringFromStd(address.address)];
                [ipv6Prefixes addObject:@(address.prefix)];
            } else {
                [ipv4Addresses addObject:NSStringFromStd(address.address)];
                [ipv4Masks addObject:IPv4Mask(address.prefix)];
            }
        }

        if (ipv4Addresses.count > 0) {
            NEIPv4Settings *ipv4 = [[NEIPv4Settings alloc] initWithAddresses:ipv4Addresses
                                                                subnetMasks:ipv4Masks];
            NSMutableArray<NEIPv4Route *> *included = [NSMutableArray array];
            NSMutableArray<NEIPv4Route *> *excluded = [NSMutableArray array];
            if (reroute_ipv4_) {
                [included addObject:[NEIPv4Route defaultRoute]];
            }
            for (const TunnelRoute &route : routes_) {
                if (!route.ipv6) {
                    [included addObject:[[NEIPv4Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(route.address)
                                                    subnetMask:IPv4Mask(route.prefix)]];
                }
            }
            for (const TunnelRoute &route : excluded_routes_) {
                if (!route.ipv6) {
                    [excluded addObject:[[NEIPv4Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(route.address)
                                                    subnetMask:IPv4Mask(route.prefix)]];
                }
            }
            // Exclude the VPN peer from a full-tunnel redirect so the control
            // channel cannot lock itself into the utun (socket_protect is a no-op
            // under Network Extension; NE exclude routes are the platform fix).
            if (reroute_ipv4_ && !remote_address_.empty()) {
                struct in_addr peer = {};
                if (inet_pton(AF_INET, remote_address_.c_str(), &peer) == 1) {
                    [excluded addObject:[[NEIPv4Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(remote_address_)
                                                    subnetMask:@"255.255.255.255"]];
                }
            }
            ipv4.includedRoutes = included;
            ipv4.excludedRoutes = excluded;
            settings.IPv4Settings = ipv4;
        }

        if (ipv6Addresses.count > 0) {
            NEIPv6Settings *ipv6 = [[NEIPv6Settings alloc] initWithAddresses:ipv6Addresses
                                                       networkPrefixLengths:ipv6Prefixes];
            NSMutableArray<NEIPv6Route *> *included = [NSMutableArray array];
            NSMutableArray<NEIPv6Route *> *excluded = [NSMutableArray array];
            if (reroute_ipv6_) {
                [included addObject:[NEIPv6Route defaultRoute]];
            }
            for (const TunnelRoute &route : routes_) {
                if (route.ipv6) {
                    [included addObject:[[NEIPv6Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(route.address)
                                                   networkPrefixLength:@(route.prefix)]];
                }
            }
            for (const TunnelRoute &route : excluded_routes_) {
                if (route.ipv6) {
                    [excluded addObject:[[NEIPv6Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(route.address)
                                                   networkPrefixLength:@(route.prefix)]];
                }
            }
            if (reroute_ipv6_ && !remote_address_.empty()) {
                struct in6_addr peer6 = {};
                if (inet_pton(AF_INET6, remote_address_.c_str(), &peer6) == 1) {
                    [excluded addObject:[[NEIPv6Route alloc]
                                            initWithDestinationAddress:NSStringFromStd(remote_address_)
                                                   networkPrefixLength:@(128)]];
                }
            }
            ipv6.includedRoutes = included;
            ipv6.excludedRoutes = excluded;
            settings.IPv6Settings = ipv6;
        }

        NSMutableArray<NSString *> *servers = [NSMutableArray array];
        NSMutableArray<NSString *> *domains = [NSMutableArray array];
        NSMutableArray<NSString *> *searchDomains = [NSMutableArray array];
        for (const std::string &server : dns_servers_) {
            AppendUnique(servers, NSStringFromStd(server));
        }
        for (const std::string &domain : dns_domains_) {
            AppendUnique(domains, NSStringFromStd(domain));
        }
        for (const std::string &domain : dns_search_domains_) {
            NSString *value = NSStringFromStd(domain);
            AppendUnique(domains, value);
            AppendUnique(searchDomains, value);
        }
        for (NSString *directive in fallback_dns_intent_) {
            NSArray<NSString *> *parts = [directive componentsSeparatedByCharactersInSet:
                [NSCharacterSet whitespaceCharacterSet]];
            NSMutableArray<NSString *> *tokens = [NSMutableArray array];
            for (NSString *part in parts) {
                if (part.length > 0) [tokens addObject:part];
            }
            if (tokens.count == 2 && [tokens[0] caseInsensitiveCompare:@"DNS"] == NSOrderedSame) {
                AppendUnique(servers, tokens[1]);
            } else if (tokens.count == 2 &&
                       [tokens[0] caseInsensitiveCompare:@"DOMAIN"] == NSOrderedSame) {
                AppendUnique(domains, tokens[1]);
            } else if (tokens.count > 1 &&
                       [tokens[0] caseInsensitiveCompare:@"DOMAIN-SEARCH"] == NSOrderedSame) {
                for (NSUInteger i = 1; i < tokens.count; i++) {
                    AppendUnique(domains, tokens[i]);
                    AppendUnique(searchDomains, tokens[i]);
                }
            }
        }

        if (![dns_policy_ isEqualToString:@"observeOnly"] && servers.count > 0) {
            NEDNSSettings *dns = [[NEDNSSettings alloc] initWithServers:servers];
            dns.searchDomains = searchDomains;
            if ([dns_policy_ isEqualToString:@"fullOverride"]) {
                dns.matchDomains = @[@""];
            } else if (domains.count > 0) {
                dns.matchDomains = domains;
                dns.matchDomainsNoSearch = YES;
            } else {
                // A split resolver without domains would become a global resolver.
                // Leave DNS untouched instead of silently widening its scope.
                dns = nil;
                [provider openVPNLog:@"Split DNS requested without match domains; macOS DNS settings were left unchanged"];
            }
            settings.DNSSettings = dns;
        }
        if (mtu_ > 0) {
            settings.MTU = @(mtu_);
        }

        NSString *error = nil;
        return [provider establishPacketBridgeWithSettings:settings error:&error];
    }

    bool socket_protect(openvpn_io::detail::socket_type socket,
                        std::string remote,
                        bool ipv6) override {
        // Network Extension packet tunnels cannot mark sockets with the classic
        // protect(fd) ioctl. Peer exclusion is applied in tun_builder_establish
        // via NEIPv4/IPv6 excludedRoutes for the tunnel remote address.
        (void)socket;
        (void)remote;
        (void)ipv6;
        return true;
    }

    bool pause_on_connection_timeout() override { return false; }

    void event(const openvpn::ClientAPI::Event &event) override {
        OpenWrapPacketTunnelProvider *provider = provider_;
        [provider openVPNEventName:NSStringFromStd(event.name)
                             info:NSStringFromStd(event.info)
                            error:event.error
                            fatal:event.fatal];
    }

    void acc_event(const openvpn::ClientAPI::AppCustomControlMessageEvent &event) override {}

    void log(const openvpn::ClientAPI::LogInfo &log) override {
        OpenWrapPacketTunnelProvider *provider = provider_;
        [provider openVPNLog:NSStringFromStd(log.text)];
    }

    void external_pki_cert_request(openvpn::ClientAPI::ExternalPKICertRequest &request) override {
        request.error = true;
        request.errorText = "External PKI profiles are not supported";
    }

    void external_pki_sign_request(openvpn::ClientAPI::ExternalPKISignRequest &request) override {
        request.error = true;
        request.errorText = "External PKI profiles are not supported";
    }

  private:
    __weak OpenWrapPacketTunnelProvider *provider_;
    NSString *dns_policy_;
    NSArray<NSString *> *fallback_dns_intent_;
    std::string remote_address_;
    std::vector<TunnelAddress> addresses_;
    std::vector<TunnelRoute> routes_;
    std::vector<TunnelRoute> excluded_routes_;
    std::vector<std::string> dns_servers_;
    std::vector<std::string> dns_domains_;
    std::vector<std::string> dns_search_domains_;
    bool reroute_ipv4_ = false;
    bool reroute_ipv6_ = false;
    int mtu_ = 0;
};

@interface OpenWrapPacketTunnelProvider () {
    std::shared_ptr<OpenWrapVPNClient> _client;
    dispatch_queue_t _workerQueue;
    dispatch_queue_t _packetQueue;
    dispatch_queue_t _lifecycleQueue;
    dispatch_queue_t _watchdogQueue;
    dispatch_queue_t _hostIpcQueue;
    dispatch_source_t _packetSource;
    dispatch_source_t _watchdogTimer;
    int _packetSocket;
    BOOL _bridgeActive;
    BOOL _stopRequested;
    BOOL _startCompletionSent;
    void (^_startCompletion)(NSError *error);
    std::atomic<double> _lastHeartbeat;
    NSMutableArray<NSString *> *_pendingHostLogs;
    NSString *_lastErrorMessage;
}
@property(nonatomic, strong) NSURL *runtimeDirectory;
- (void)completeStartWithError:(NSError *)error cancelTunnelIfAlreadyStarted:(BOOL)cancelTunnel;
- (void)enqueueHostLog:(NSString *)line;
- (void)recordProviderError:(NSString *)message;
@end

@implementation OpenWrapPacketTunnelProvider

- (instancetype)init {
    self = [super init];
    if (self) {
        _workerQueue = dispatch_queue_create("app.openwrap.packet-tunnel.openvpn", DISPATCH_QUEUE_SERIAL);
        _packetQueue = dispatch_queue_create("app.openwrap.packet-tunnel.packets", DISPATCH_QUEUE_SERIAL);
        dispatch_queue_set_specific(_packetQueue, &PacketQueueKey, &PacketQueueKey, NULL);
        _lifecycleQueue = dispatch_queue_create("app.openwrap.packet-tunnel.lifecycle", DISPATCH_QUEUE_SERIAL);
        _watchdogQueue = dispatch_queue_create("app.openwrap.packet-tunnel.watchdog", DISPATCH_QUEUE_SERIAL);
        _hostIpcQueue = dispatch_queue_create("app.openwrap.packet-tunnel.host-ipc", DISPATCH_QUEUE_SERIAL);
        _packetSocket = -1;
        _pendingHostLogs = [NSMutableArray array];
        _lastErrorMessage = nil;
    }
    return self;
}

- (void)startTunnelWithOptions:(NSDictionary<NSString *, NSObject *> *)options
             completionHandler:(void (^)(NSError *error))completionHandler {
    dispatch_sync(_lifecycleQueue, ^{
      self->_stopRequested = NO;
      self->_startCompletion = [completionHandler copy];
      self->_startCompletionSent = NO;
    });

    NSData *payloadData = (NSData *)options[@"openwrapPayload"];
    if (![payloadData isKindOfClass:[NSData class]]) {
        NSError *error = [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                              code:1
                                          userInfo:@{NSLocalizedDescriptionKey : @"Missing OpenWrap tunnel payload"}];
        [self completeStartWithError:error cancelTunnelIfAlreadyStarted:NO];
        return;
    }

    NSError *jsonError = nil;
    NSDictionary *payload = [NSJSONSerialization JSONObjectWithData:payloadData
                                                            options:0
                                                              error:&jsonError];
    if (![payload isKindOfClass:[NSDictionary class]]) {
        NSError *error = jsonError ?: [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                                            code:2
                                                        userInfo:@{NSLocalizedDescriptionKey : @"Invalid OpenWrap tunnel payload"}];
        [self completeStartWithError:error cancelTunnelIfAlreadyStarted:NO];
        return;
    }

    NSError *materializeError = nil;
    NSString *config = [self materializePayload:payload error:&materializeError];
    if (config == nil) {
        [self completeStartWithError:materializeError cancelTunnelIfAlreadyStarted:NO];
        return;
    }

    _lastHeartbeat.store(CFAbsoluteTimeGetCurrent());
    NSString *dnsPolicy = [payload[@"dnsPolicy"] isKindOfClass:[NSString class]]
                              ? payload[@"dnsPolicy"]
                              : @"splitDnsPreferred";
    NSArray<NSString *> *dnsIntent = [payload[@"dnsIntent"] isKindOfClass:[NSArray class]]
                                         ? payload[@"dnsIntent"]
                                         : @[];
    NSString *username = [payload[@"username"] isKindOfClass:[NSString class]]
                             ? payload[@"username"]
                             : nil;
    NSString *password = [payload[@"password"] isKindOfClass:[NSString class]]
                             ? payload[@"password"]
                             : nil;
    auto material = std::make_shared<StartMaterial>();
    material->config = std::string(config.UTF8String ?: "");
    material->username = std::string(username.UTF8String ?: "");
    material->password = std::string(password.UTF8String ?: "");

    std::shared_ptr<OpenWrapVPNClient> client =
        std::make_shared<OpenWrapVPNClient>(self, dnsPolicy, dnsIntent);
    __block BOOL shouldStart = NO;
    dispatch_sync(_lifecycleQueue, ^{
      if (!self->_stopRequested) {
          self->_client = client;
          [self startHostWatchdog];
          shouldStart = YES;
      }
    });
    if (!shouldStart) {
        material->clear();
        return;
    }
    dispatch_async(_workerQueue, ^{
      __block BOOL stopped = NO;
      dispatch_sync(self->_lifecycleQueue, ^{
        stopped = self->_stopRequested;
      });
      if (stopped) {
          material->clear();
          return;
      }
      openvpn::ClientAPI::Config clientConfig;
      clientConfig.content = std::move(material->config);
      clientConfig.guiVersion = "OpenWrap 0.1.0";
      clientConfig.tunPersist = false;
      clientConfig.connTimeout = 30;
      openvpn::ClientAPI::EvalConfig evaluation = client->eval_config(clientConfig);
      SecureClear(clientConfig.content);
      if (evaluation.error) {
          material->clear();
          [self finishStartWithErrorMessage:NSStringFromStd(evaluation.message)];
          return;
      }
      if (!evaluation.autologin) {
          openvpn::ClientAPI::ProvideCreds creds;
          creds.username = std::move(material->username);
          creds.password = std::move(material->password);
          openvpn::ClientAPI::Status credentialStatus = client->provide_creds(creds);
          SecureClear(creds.username);
          SecureClear(creds.password);
          if (credentialStatus.error) {
              material->clear();
              [self finishStartWithErrorMessage:NSStringFromStd(credentialStatus.message)];
              return;
          }
      }
      material->clear();
      openvpn::ClientAPI::Status status = client->connect();
      if (status.error) {
          [self finishStartWithErrorMessage:NSStringFromStd(status.message)];
          return;
      }
      // connect() returned without an OpenVPN error: the session ended (remote
      // close, clean shutdown, or host stop). Tear the NE tunnel down so macOS
      // does not keep advertising a dead Connected data path.
      __block BOOL stopRequested = NO;
      dispatch_sync(self->_lifecycleQueue, ^{
        stopRequested = self->_stopRequested;
      });
      if (!stopRequested) {
          [self enqueueHostLog:@"OpenVPN session ended; stopping Packet Tunnel provider"];
          dispatch_async(self->_lifecycleQueue, ^{
            if (!self->_stopRequested) {
                [self cancelTunnelWithError:nil];
            }
          });
      }
    });
}

- (void)stopTunnelWithReason:(NEProviderStopReason)reason
           completionHandler:(void (^)(void))completionHandler {
    __block std::shared_ptr<OpenWrapVPNClient> client;
    NSError *startError = [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                               code:5
                                           userInfo:@{NSLocalizedDescriptionKey : @"Tunnel start was cancelled"}];
    __block void (^pendingStartCompletion)(NSError *) = nil;
    dispatch_sync(_lifecycleQueue, ^{
      self->_stopRequested = YES;
      client = self->_client;
      self->_client.reset();
      [self stopHostWatchdog];
      if (!self->_startCompletionSent && self->_startCompletion != nil) {
          self->_startCompletionSent = YES;
          pendingStartCompletion = self->_startCompletion;
          self->_startCompletion = nil;
      }
    });
    if (client) {
        client->stop();
    }
    [self stopPacketBridge];
    if (pendingStartCompletion != nil) {
        pendingStartCompletion(startError);
    }
    if (self.runtimeDirectory != nil) {
        [[NSFileManager defaultManager] removeItemAtURL:self.runtimeDirectory error:nil];
        self.runtimeDirectory = nil;
    }
    completionHandler();
}

- (void)handleAppMessage:(NSData *)messageData
       completionHandler:(void (^)(NSData *responseData))completionHandler {
    NSString *message = [[NSString alloc] initWithData:messageData encoding:NSUTF8StringEncoding];
    if ([message isEqualToString:@"heartbeat"] || [message isEqualToString:@"status"]) {
        _lastHeartbeat.store(CFAbsoluteTimeGetCurrent());
        __block NSArray<NSString *> *logs = @[];
        __block NSString *lastError = nil;
        dispatch_sync(_hostIpcQueue, ^{
          logs = [self->_pendingHostLogs copy] ?: @[];
          [self->_pendingHostLogs removeAllObjects];
          lastError = [self->_lastErrorMessage copy];
        });
        NSMutableDictionary *payload = [@{
            @"ok" : @YES,
            @"logs" : logs,
        } mutableCopy];
        if (lastError.length > 0) {
            payload[@"lastError"] = lastError;
        } else {
            payload[@"lastError"] = [NSNull null];
        }
        NSError *jsonError = nil;
        NSData *response = [NSJSONSerialization dataWithJSONObject:payload
                                                           options:0
                                                             error:&jsonError];
        if (response == nil) {
            completionHandler([@"ok" dataUsingEncoding:NSUTF8StringEncoding]);
            return;
        }
        completionHandler(response);
    } else {
        completionHandler(nil);
    }
}

- (void)startHostWatchdog {
    [self stopHostWatchdog];
    _watchdogTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER,
                                            0,
                                            0,
                                            _watchdogQueue);
    dispatch_source_set_timer(_watchdogTimer,
                              dispatch_time(DISPATCH_TIME_NOW, 20 * NSEC_PER_SEC),
                              5 * NSEC_PER_SEC,
                              NSEC_PER_SEC);
    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(_watchdogTimer, ^{
      typeof(self) self = weakSelf;
      if (self == nil) return;
      double elapsed = CFAbsoluteTimeGetCurrent() - self->_lastHeartbeat.load();
      if (elapsed > 20.0) {
          NSError *error = [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                               code:6
                                           userInfo:@{NSLocalizedDescriptionKey : @"OpenWrap host heartbeat stopped"}];
          dispatch_async(dispatch_get_main_queue(), ^{
            [self cancelTunnelWithError:error];
          });
      }
    });
    dispatch_resume(_watchdogTimer);
}

- (void)stopHostWatchdog {
    if (_watchdogTimer != nil) {
        dispatch_source_cancel(_watchdogTimer);
        _watchdogTimer = nil;
    }
}

- (NSString *)materializePayload:(NSDictionary *)payload error:(NSError **)error {
    NSString *config = [payload[@"config"] isKindOfClass:[NSString class]] ? payload[@"config"] : nil;
    NSDictionary *assets = [payload[@"assets"] isKindOfClass:[NSDictionary class]] ? payload[@"assets"] : @{};
    if (config == nil) {
        if (error) {
            *error = [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                         code:3
                                     userInfo:@{NSLocalizedDescriptionKey : @"Tunnel profile is missing"}];
        }
        return nil;
    }

    NSURL *root = [[NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES]
        URLByAppendingPathComponent:[[NSUUID UUID] UUIDString]
                         isDirectory:YES];
    if (![[NSFileManager defaultManager] createDirectoryAtURL:root
                                  withIntermediateDirectories:YES
                                                   attributes:@{NSFilePosixPermissions : @0700}
                                                        error:error]) {
        return nil;
    }
    self.runtimeDirectory = root;
    NSString *rewritten = [config copy];
    for (NSString *relativePath in assets) {
        if (![relativePath isKindOfClass:[NSString class]]) {
            continue;
        }
        NSArray<NSString *> *components = [relativePath pathComponents];
        if (components.count != 2 ||
            ![components[0] isEqualToString:@"assets"] ||
            [relativePath containsString:@".."] || [relativePath hasPrefix:@"/"]) {
            continue;
        }
        NSData *data = nil;
        id assetValue = assets[relativePath];
        if ([assetValue isKindOfClass:[NSString class]]) {
            // Preferred: base64-encoded asset bytes from the host bridge.
            data = [[NSData alloc] initWithBase64EncodedString:(NSString *)assetValue options:0];
        } else if ([assetValue isKindOfClass:[NSArray class]]) {
            // Legacy fallback: JSON array of byte numbers.
            NSArray *bytes = (NSArray *)assetValue;
            NSMutableData *buffer = [NSMutableData dataWithCapacity:bytes.count];
            for (id number in bytes) {
                if (![number isKindOfClass:[NSNumber class]]) {
                    continue;
                }
                uint8_t value = [(NSNumber *)number unsignedCharValue];
                [buffer appendBytes:&value length:1];
            }
            data = buffer;
        }
        if (data == nil) {
            continue;
        }
        NSURL *target = [root URLByAppendingPathComponent:relativePath];
        if (![[NSFileManager defaultManager]
                createDirectoryAtURL:[target URLByDeletingLastPathComponent]
          withIntermediateDirectories:YES
                           attributes:@{NSFilePosixPermissions : @0700}
                                error:error] ||
            ![data writeToURL:target options:NSDataWritingAtomic error:error] ||
            ![[NSFileManager defaultManager]
                setAttributes:@{NSFilePosixPermissions : @0600}
                 ofItemAtPath:target.path
                        error:error]) {
            return nil;
        }
        rewritten = RewriteAssetReference(rewritten, relativePath, target.path);
    }
    return rewritten;
}

- (int)establishPacketBridgeWithSettings:(NEPacketTunnelNetworkSettings *)settings
                                    error:(NSString **)errorMessage {
    dispatch_semaphore_t semaphore = dispatch_semaphore_create(0);
    __block NSError *settingsError = nil;
    [self setTunnelNetworkSettings:settings
                 completionHandler:^(NSError *error) {
                   settingsError = error;
                   dispatch_semaphore_signal(semaphore);
                 }];
    if (dispatch_semaphore_wait(semaphore,
                                dispatch_time(DISPATCH_TIME_NOW, 15 * NSEC_PER_SEC)) != 0) {
        if (errorMessage) *errorMessage = @"Timed out applying Packet Tunnel settings";
        return -1;
    }
    if (settingsError != nil) {
        if (errorMessage) *errorMessage = settingsError.localizedDescription;
        return -1;
    }

    int sockets[2] = {-1, -1};
    if (socketpair(PF_LOCAL, SOCK_DGRAM, IPPROTO_IP, sockets) != 0) {
        if (errorMessage) *errorMessage = [NSString stringWithUTF8String:strerror(errno)];
        return -1;
    }
    int bufferSize = 1024 * 1024;
    setsockopt(sockets[0], SOL_SOCKET, SO_RCVBUF, &bufferSize, sizeof(bufferSize));
    setsockopt(sockets[0], SOL_SOCKET, SO_SNDBUF, &bufferSize, sizeof(bufferSize));
    setsockopt(sockets[1], SOL_SOCKET, SO_RCVBUF, &bufferSize, sizeof(bufferSize));
    setsockopt(sockets[1], SOL_SOCKET, SO_SNDBUF, &bufferSize, sizeof(bufferSize));
    fcntl(sockets[0], F_SETFL, fcntl(sockets[0], F_GETFL) | O_NONBLOCK);
    int providerSocket = sockets[0];
    [self stopPacketBridge];
    __block BOOL bridgeEstablished = NO;
    dispatch_sync(_lifecycleQueue, ^{
      if (!self->_stopRequested) {
          dispatch_sync(self->_packetQueue, ^{
            self->_packetSocket = providerSocket;
            self->_bridgeActive = YES;
            [self startReadingCoreSocket];
          });
          bridgeEstablished = YES;
      }
    });
    if (!bridgeEstablished) {
        close(sockets[0]);
        close(sockets[1]);
        if (errorMessage) *errorMessage = @"Tunnel was stopped before packet flow was established";
        return -1;
    }
    [self startReadingPacketFlow];
    return sockets[1];
}

- (void)startReadingPacketFlow {
    __weak typeof(self) weakSelf = self;
    [self.packetFlow readPacketsWithCompletionHandler:^(NSArray<NSData *> *packets,
                                                        NSArray<NSNumber *> *protocols) {
      typeof(self) self = weakSelf;
      if (self == nil) return;
      dispatch_async(self->_packetQueue, ^{
        if (!self->_bridgeActive || self->_packetSocket < 0) return;
        for (NSUInteger index = 0; index < packets.count && index < protocols.count; index++) {
            uint32_t family = htonl(protocols[index].unsignedIntValue);
            NSMutableData *framed = [NSMutableData dataWithBytes:&family length:sizeof(family)];
            [framed appendData:packets[index]];
            send(self->_packetSocket, framed.bytes, framed.length, 0);
        }
        [self startReadingPacketFlow];
      });
    }];
}

- (void)startReadingCoreSocket {
    if (_packetSource != nil || _packetSocket < 0) return;
    _packetSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_READ,
                                           (uintptr_t)_packetSocket,
                                           0,
                                           _packetQueue);
    __weak typeof(self) weakSelf = self;
    dispatch_source_set_event_handler(_packetSource, ^{
      typeof(self) self = weakSelf;
      if (self == nil || !self->_bridgeActive) return;
      uint8_t buffer[65536];
      ssize_t length = recv(self->_packetSocket, buffer, sizeof(buffer), 0);
      if (length <= (ssize_t)sizeof(uint32_t)) return;
      uint32_t networkFamily = 0;
      memcpy(&networkFamily, buffer, sizeof(networkFamily));
      NSNumber *family = @(ntohl(networkFamily));
      NSData *packet = [NSData dataWithBytes:buffer + sizeof(networkFamily)
                                      length:(NSUInteger)length - sizeof(networkFamily)];
      [self.packetFlow writePackets:@[ packet ] withProtocols:@[ family ]];
    });
    dispatch_resume(_packetSource);
}

- (void)stopPacketBridge {
    void (^stopBlock)(void) = ^{
      self->_bridgeActive = NO;
      if (self->_packetSource != nil) {
          dispatch_source_cancel(self->_packetSource);
          self->_packetSource = nil;
      }
      if (self->_packetSocket >= 0) {
          close(self->_packetSocket);
          self->_packetSocket = -1;
      }
    };
    if (dispatch_get_specific(&PacketQueueKey) == &PacketQueueKey) {
        stopBlock();
    } else {
        dispatch_sync(_packetQueue, stopBlock);
    }
}

- (void)enqueueHostLog:(NSString *)line {
    if (line.length == 0) {
        return;
    }
    dispatch_async(_hostIpcQueue, ^{
      [self->_pendingHostLogs addObject:line];
      // Cap buffer so a runaway log stream cannot unbounded-grow the extension.
      while (self->_pendingHostLogs.count > 500) {
          [self->_pendingHostLogs removeObjectAtIndex:0];
      }
    });
}

- (void)recordProviderError:(NSString *)message {
    if (message.length == 0) {
        return;
    }
    dispatch_async(_hostIpcQueue, ^{
      self->_lastErrorMessage = [message copy];
    });
    [self enqueueHostLog:message];
}

- (void)openVPNEventName:(NSString *)name
                    info:(NSString *)info
                   error:(BOOL)isError
                   fatal:(BOOL)isFatal {
    NSString *line = [NSString stringWithFormat:@"EVENT: %@ %@", name ?: @"", info ?: @""];
    [self enqueueHostLog:line];
    NSLog(@"OpenWrap OpenVPN event %@: %@", name, info);
    if ([name isEqualToString:@"CONNECTED"]) {
        [self completeStartWithError:nil cancelTunnelIfAlreadyStarted:NO];
    } else if (isFatal) {
        NSString *detail = info.length > 0 ? info : name;
        [self recordProviderError:detail ?: @"OpenVPN fatal event"];
        [self finishStartWithErrorMessage:detail];
    } else if (isError) {
        [self recordProviderError:info.length > 0 ? info : name];
    }
}

- (void)openVPNLog:(NSString *)line {
    NSLog(@"OpenWrap OpenVPN: %@", line);
    [self enqueueHostLog:line];
}

- (void)finishStartWithErrorMessage:(NSString *)message {
    [self recordProviderError:message ?: @"OpenVPN connection failed"];
    NSError *error = [NSError errorWithDomain:@"OpenWrapPacketTunnel"
                                         code:4
                                     userInfo:@{NSLocalizedDescriptionKey : message ?: @"OpenVPN connection failed"}];
    [self completeStartWithError:error cancelTunnelIfAlreadyStarted:YES];
}

- (void)completeStartWithError:(NSError *)error cancelTunnelIfAlreadyStarted:(BOOL)cancelTunnel {
    dispatch_async(_lifecycleQueue, ^{
      if (!self->_startCompletionSent && self->_startCompletion != nil) {
          self->_startCompletionSent = YES;
          void (^completion)(NSError *) = self->_startCompletion;
          self->_startCompletion = nil;
          completion(error);
      } else if (error != nil && cancelTunnel && !self->_stopRequested) {
          [self cancelTunnelWithError:error];
      }
    });
}

@end
