OpenWrap — personal share build
==============================

This folder contains OpenWrap, a macOS OpenVPN client (hobby project, MIT
license). The app is UNSIGNED: it is a personal build shared directly with
you, not something from the App Store or a verified developer. Only install
it if you trust the person who sent it to you.

Requirements
------------
- Apple Silicon Mac (M1/M2/M3/M4). This build will NOT run on Intel Macs.
- An administrator account.
- OpenVPN — the setup script checks for it and can install it via Homebrew.

Setup (one step)
----------------
Double-click "setup.sh" (it opens in Terminal), or open Terminal and run:

    cd ~/Downloads/OpenWrap-Share     # wherever you unzipped it
    ./setup.sh

The script will:
  1. Clear the macOS quarantine flag so the app opens without warnings.
  2. Check for OpenVPN and offer to install it with Homebrew if missing.
  3. Install the privileged helper that OpenWrap uses to start OpenVPN
     (in /Library/PrivilegedHelperTools). It will ask for your password.

After setup
-----------
Drag OpenWrap.app into /Applications (optional), open it, import a .ovpn
profile, and connect.

Updates
-------
You only need to do this setup once. OpenWrap checks GitHub for a new version
every time it starts and offers "Update and restart" (Settings > Updates has a
manual check). Updates are verified with the project's signing key.

If an update ships a new privileged helper, OpenWrap shows a banner with one
Terminal command to run (it asks for your password).

Latest version: https://github.com/vinicius741/openwrap/releases/latest

Uninstall
---------
  sudo rm /Library/PrivilegedHelperTools/app.openwrap.desktop.openwrap-helper
  sudo rm -rf /Applications/OpenWrap.app
  rm -rf ~/Library/Application\ Support/OpenWrap    # profiles, logs, settings
