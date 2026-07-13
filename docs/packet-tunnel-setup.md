# Packet Tunnel Setup

> **Hobby builds: skip this document.**  
> The default, supported path is the free setuid helper — [Helper Setup](helper-setup.md).  
> Packet Tunnel requires an **Apple Developer Program membership** (paid annual fee) with Network Extension entitlements. There is **no free workaround** for activating system extensions on modern macOS.  
> This project intentionally keeps Packet Tunnel **opt-in** so unpaid hobby use keeps working. Do not make this path the default in reviews or refactors (`AGENTS.md` → “Hobby constraint”).

OpenWrap can optionally use a Packet Tunnel Network Extension packaged as a system extension (`OPENWRAP_VPN_BACKEND=packet-tunnel`). It embeds OpenVPN 3 Core and does not launch a Homebrew OpenVPN binary, install a setuid helper, call `networksetup`, or write `/etc/resolver`.

## Why a system extension

OpenWrap is intended for direct distribution. Apple requires a packet tunnel provider distributed outside the Mac App Store to use the system-extension packaging model. The extension is embedded at:

`OpenWrap.app/Contents/Library/SystemExtensions/app.openwrap.desktop.PacketTunnel.systemextension`

The host activates it with `OSSystemExtensionRequest`, then saves a minimal `NETunnelProviderManager` record. Profile contents, copied certificate/key assets, username, and password are supplied only as start options for the active connection.

## Apple configuration

The Apple Developer account must have Network Extensions access for both identifiers:

- Host app: `app.openwrap.desktop`
- Provider: `app.openwrap.desktop.PacketTunnel`

The host needs the System Extension install entitlement. The provider needs `packet-tunnel-provider-systemextension`, App Sandbox, and outbound network access. The checked-in entitlement files describe these rights, but Apple must authorize them in the signing profiles; adding the XML alone is not sufficient.

Create the required Developer ID provisioning profiles and set:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Company (TEAMID)"
export OPENWRAP_APP_PROVISIONING_PROFILE="/absolute/path/to/host-app.provisionprofile"
export OPENWRAP_PACKET_TUNNEL_PROVISIONING_PROFILE="/absolute/path/to/provider.provisionprofile"
```

Both provisioning profiles are required because the host and provider use Apple-restricted entitlements. The signed build script embeds them before its final signing verification.

## Build

```bash
npm install
npm run tauri:build:signed
```

The native script downloads pinned source revisions of OpenVPN 3 Core, Asio, fmt, LZ4, and Mbed TLS. OpenVPN 3 Core is used under its MPL-2.0 option; its license is copied into the system extension's resources.

An unsigned `npm run tauri:build` remains useful for compilation checks only. macOS will reject activation until the system extension and containing app are correctly provisioned and signed. End-to-end testing must use the app produced by `npm run tauri:build:signed`, not `tauri dev`.

The signed build ends by checking the embedded profiles, both signatures, both required entitlements, and matching Team IDs. To verify an existing bundle independently:

```bash
./scripts/verify-network-extension-signing.sh
```

## First run

The first connection submits a system-extension activation request. macOS may ask the user to approve OpenWrap in System Settings. After approval, reconnect if the original request did not continue automatically.

## Verification

Confirm the extension is active:

```bash
systemextensionsctl list | grep app.openwrap.desktop.PacketTunnel
```

While connected, inspect macOS's effective resolver and routes:

```bash
scutil --dns
route -n get default
```

Test the failure mode that motivated this migration:

1. Record `scutil --dns` before connecting.
2. Connect a profile with `FullOverride` or domain-scoped split DNS.
3. Confirm the expected resolver appears while the tunnel is connected.
4. Force-terminate the Packet Tunnel provider process.
5. Confirm the tunnel disconnects and the resolver disappears without reopening OpenWrap or editing System Settings.

## DNS behavior

The provider applies DNS only through `NEPacketTunnelNetworkSettings.DNSSettings`:

- `ObserveOnly`: no resolver is installed.
- `SplitDnsPreferred`: resolver servers are scoped with `matchDomains`. A profile without match domains does not silently become a global override.
- `FullOverride`: `matchDomains = [""]`, which makes the tunnel resolver eligible for all names while the tunnel is active.

No restore snapshot or crash cleanup routine is needed because macOS associates these settings with the provider's virtual interface.

OpenWrap additionally uses a host heartbeat. If the desktop process crashes, the provider detects the missing heartbeat within about 20 seconds and cancels the tunnel, which asks macOS to tear down all associated settings.
