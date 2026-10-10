# ⚡ ADB Bridge GUI (Pure Rust)

A native, ultra-fast Windows desktop application for managing Android devices wirelessly without ever typing `adb connect` in a terminal again.

---

## 🚀 Features

### 1. 📱 Wireless Connect & History
- **Quick Connect**: Enter IP and port (defaults to `5555`) and connect/disconnect with one click.
- **Android 11+ Wireless Pairing**: Dedicated interface to enter pairing IP:port and the 6-digit code shown on your phone.
- **USB to Wireless Mode**: Click *Enable Wireless on USB (`adb tcpip 5555`)* to switch a plugged-in phone to Wi-Fi mode seamlessly.
- **Saved Devices & Favorites**: Automatically remembers previously connected devices with custom nicknames (e.g., "Pixel 8", "Living Room TV") for 1-click reconnecting.

### 2. 💻 Connected Devices & Quick Actions
- **Live Device Table**: Displays serial, model name, connection type (Wi-Fi or USB), status badge (Online, Unauthorized, Offline), and live battery percentage.
- **🔄 Reverse TCP Port Manager (`adb reverse tcp:`)**:
  - Route connections from Android `localhost` to your computer's `localhost` with 1 click.
  - Custom Device Port and Host Port input fields (e.g. `tcp:8081` ➔ `tcp:8081`).
  - Quick 1-click Preset chips:
    - **`8081 (Metro)`** (React Native / Expo)
    - **`3000 (React/Node)`** (Web frontends / backends)
    - **`8080 (Server)`** (Spring Boot / API services)
    - **`5173 (Vite)`** (Vite dev server)
  - Live Active Reverse Rules list with individual **`[Remove]`** and **`[Remove All]`** buttons.
- **📺 Mirror Screen (`scrcpy`)**: 1-click screen mirroring directly from the device card.
- **📦 Install APK**: Native Windows file dialog to select and install `.apk` files without terminal commands.
- **📸 Take Screenshot**: Captures phone screen and saves PNG to your chosen location.
- **🔄 Reboot Menu**: Fast reboot to Normal, Recovery, or Bootloader/Fastboot.

### 3. 🔍 Local Subnet Auto-Scanner
- Auto-detects your local Wi-Fi subnet (e.g. `192.168.1.x`).
- Multi-threaded scanner probes port `5555` across all host IPs in seconds.
- Discovered devices appear with instant 1-click `Connect` buttons.

### 4. ⚙ Settings & Console
- Auto-detects ADB (verified at `C:\Android\sdk\platform-tools\adb.exe`) or allows custom path overrides.
- Custom ADB command prompt for running raw adb arguments with live output.
- Real-time collapsible console log showing all commands executed, status codes, and timestamps.

---

## 🛠 How to Run

Just double-click:
```
E:\ADB-GUI\adb_gui.exe
```
or run:
```
run.bat
```

---

## 🔨 How to Rebuild (if you modify code)

Simply run:
```cmd
build.bat
```
This builds the release binary into your system temp directory to prevent Windows file-locking issues, then places the updated `adb_gui.exe` right here in the root folder.
