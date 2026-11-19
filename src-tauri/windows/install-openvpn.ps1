# Installs OpenVPN Community for OpenWrap on Windows.
#
# Shared by the NSIS installer (post-install hook, output in the installer's
# details pane) and the in-app "Install OpenVPN" button (own console window).
# Both callers are already elevated. Installed next to openwrap-app.exe in
# C:\Program Files\OpenWrap, which only administrators can modify.
#
# Downloads a pinned OpenVPN MSI from openvpn.net, verifies its SHA-256 and
# installs it with a progress bar and no wizard clicks. Pinning (instead of
# winget) keeps the install deterministic and works on PCs without winget.
#
#   -CheckOnly      exit 0 when OpenVPN is installed, 2 when it is missing
#   -PauseOnError   wait for Enter before closing on failure (console use)
#
# Exit codes: 0 installed, 1 failed, 2 missing (-CheckOnly).
param(
    [switch]$CheckOnly,
    [switch]$PauseOnError
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Bump all three together; the hash comes from the openvpn.net download.
$OpenVpnVersion = '2.7.9'
$MsiUrl = "https://swupdate.openvpn.org/community/releases/OpenVPN-$OpenVpnVersion-I001-amd64.msi"
$MsiSha256 = '47abc38a15c217b848fe099332a8523500fcbf2ebf8a1eaa7d5648c6e75079aa'

function Find-OpenVpn {
    # Same lookup order as openwrap-core (crates/openwrap-core/src/windows.rs).
    # Read the 64-bit registry view and ProgramW6432 explicitly: the NSIS
    # installer is a 32-bit process, so this script may run in 32-bit PowerShell.
    $candidates = @()
    try {
        $hklm = [Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine', 'Registry64')
        $key = $hklm.OpenSubKey('SOFTWARE\OpenVPN')
        if ($key) {
            $exePath = $key.GetValue('exe_path')
            if ($exePath) { $candidates += $exePath }
            $installDir = $key.GetValue('')
            if ($installDir) { $candidates += (Join-Path $installDir 'bin\openvpn.exe') }
            $key.Close()
        }
    } catch {}
    foreach ($dir in @($env:ProgramW6432, $env:ProgramFiles)) {
        if ($dir) { $candidates += (Join-Path $dir 'OpenVPN\bin\openvpn.exe') }
    }
    $candidates += 'C:\Program Files\OpenVPN\bin\openvpn.exe'
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
    }
    return $null
}

function Fail([string]$message) {
    Write-Host ''
    Write-Host "OpenVPN setup failed: $message" -ForegroundColor Red
    Write-Host 'You can also install it manually from https://openvpn.net/community-downloads/'
    if ($PauseOnError) {
        Write-Host ''
        Read-Host 'Press Enter to close this window' | Out-Null
    }
    exit 1
}

$existing = Find-OpenVpn
if ($CheckOnly) {
    if ($existing) { exit 0 } else { exit 2 }
}
if ($existing) {
    Write-Host "OpenVPN is already installed: $existing"
    exit 0
}

$msi = Join-Path ([System.IO.Path]::GetTempPath()) ("openwrap-openvpn-" + [guid]::NewGuid().ToString('N') + '.msi')
try {
    Write-Host "Downloading OpenVPN Community $OpenVpnVersion from openvpn.net..."
    # Windows PowerShell 5.1 on older Windows 10 builds may not offer TLS 1.2 by default.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    try {
        Invoke-WebRequest -Uri $MsiUrl -OutFile $msi -UseBasicParsing
    } catch {
        Fail "could not download $MsiUrl ($($_.Exception.Message)). Check the internet connection and try again."
    }

    $hash = (Get-FileHash -LiteralPath $msi -Algorithm SHA256).Hash
    if ($hash -ne $MsiSha256) {
        Fail "the downloaded file does not match the expected checksum."
    }

    Write-Host "Installing OpenVPN Community $OpenVpnVersion (a progress window opens)..."
    $process = Start-Process -FilePath "$env:SystemRoot\System32\msiexec.exe" `
        -ArgumentList @('/i', "`"$msi`"", '/passive', '/norestart') -Wait -PassThru
    # 3010: installed, a restart is recommended.
    if ($process.ExitCode -ne 0 -and $process.ExitCode -ne 3010) {
        Fail "the OpenVPN installer exited with code $($process.ExitCode)."
    }
} finally {
    Remove-Item -LiteralPath $msi -Force -ErrorAction SilentlyContinue
}

$installed = Find-OpenVpn
if (-not $installed) {
    Fail 'installation finished, but openvpn.exe was not found.'
}
Write-Host "OpenVPN is installed: $installed" -ForegroundColor Green
exit 0
