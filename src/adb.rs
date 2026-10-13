use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: String,
    pub state: String,
    pub model: String,
    pub product: String,
    pub device: String,
    pub transport_id: String,
    pub is_wireless: bool,
    pub battery: Option<String>,
    pub reversed_ports: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdnsService {
    pub name: String,
    pub service_type: String,
    pub ip: String,
    pub port: u16,
}

fn clean_service_name(name: &str) -> String {
    // If name ends with " (N)" where N is collision number, strip it
    if let Some(idx) = name.rfind(" (") {
        if name.ends_with(')') {
            let inner = &name[idx + 2..name.len() - 1];
            if inner.chars().all(|c| c.is_ascii_digit()) {
                return name[..idx].trim().to_string();
            }
        }
    }
    name.to_string()
}

#[derive(Debug, Clone)]
pub struct AdbManager {
    pub custom_adb_path: Option<String>,
    pub custom_scrcpy_path: Option<String>,
}

impl AdbManager {
    pub fn new(custom_adb: Option<String>, custom_scrcpy: Option<String>) -> Self {
        Self {
            custom_adb_path: custom_adb,
            custom_scrcpy_path: custom_scrcpy,
        }
    }

    pub fn resolve_adb_path(&self) -> Option<PathBuf> {
        if let Some(ref path_str) = self.custom_adb_path {
            let p = PathBuf::from(path_str);
            if p.is_file() {
                return Some(p);
            }
        }

        // Check PATH
        if let Ok(p) = which::which("adb") {
            return Some(p);
        }

        // Check common Windows SDK locations
        let candidates = [
            r"C:\Android\sdk\platform-tools\adb.exe",
            r"C:\Program Files (x86)\Android\android-sdk\platform-tools\adb.exe",
        ];

        for c in &candidates {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Some(p);
            }
        }

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let mut p = PathBuf::from(local_app_data);
            p.push(r"Android\Sdk\platform-tools\adb.exe");
            if p.is_file() {
                return Some(p);
            }
        }

        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let mut p = PathBuf::from(user_profile);
            p.push(r"AppData\Local\Android\Sdk\platform-tools\adb.exe");
            if p.is_file() {
                return Some(p);
            }
        }

        None
    }

    pub fn resolve_scrcpy_path(&self) -> Option<PathBuf> {
        if let Some(ref path_str) = self.custom_scrcpy_path {
            let p = PathBuf::from(path_str);
            if p.is_file() {
                return Some(p);
            }
        }

        if let Ok(p) = which::which("scrcpy") {
            return Some(p);
        }

        let candidates = [
            r"C:\scrcpy\scrcpy.exe",
            r"C:\tools\scrcpy\scrcpy.exe",
            r"C:\Program Files\scrcpy\scrcpy.exe",
        ];

        for c in &candidates {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Some(p);
            }
        }

        None
    }

    pub fn run_command(&self, args: &[&str]) -> Result<String, String> {
        let adb_path = self
            .resolve_adb_path()
            .ok_or_else(|| "ADB binary not found. Please install Android Platform Tools or set path in Settings.".to_string())?;

        let mut cmd = Command::new(&adb_path);
        cmd.args(args);

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                if !output.status.success() {
                    let err_msg = if !stderr.is_empty() {
                        stderr
                    } else if !stdout.is_empty() {
                        stdout
                    } else {
                        format!("Command failed with exit code: {:?}", output.status.code())
                    };
                    Err(err_msg)
                } else if !stderr.is_empty() && stdout.is_empty() {
                    // Some adb commands output info to stderr even on success
                    Ok(stderr)
                } else {
                    Ok(stdout)
                }
            }
            Err(e) => Err(format!("Failed to execute adb: {}", e)),
        }
    }

    pub fn get_adb_version(&self) -> Result<String, String> {
        let out = self.run_command(&["version"])?;
        let first_line = out.lines().next().unwrap_or("Unknown version");
        Ok(first_line.to_string())
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceInfo>, String> {
        let out = self.run_command(&["devices", "-l"])?;
        let mut devices = Vec::new();

        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of devices attached") || line.starts_with('*') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let serial = parts[0].to_string();
            let state = parts[1].to_string();

            let mut model = String::new();
            let mut product = String::new();
            let mut device = String::new();
            let mut transport_id = String::new();

            for part in &parts[2..] {
                if let Some(m) = part.strip_prefix("model:") {
                    model = m.replace('_', " ");
                } else if let Some(p) = part.strip_prefix("product:") {
                    product = p.to_string();
                } else if let Some(d) = part.strip_prefix("device:") {
                    device = d.to_string();
                } else if let Some(t) = part.strip_prefix("transport_id:") {
                    transport_id = t.to_string();
                }
            }

            if model.is_empty() {
                model = if product.is_empty() {
                    serial.clone()
                } else {
                    product.clone()
                };
            }

            let is_wireless = serial.contains(':');

            devices.push(DeviceInfo {
                serial,
                state,
                model,
                product,
                device,
                transport_id,
                is_wireless,
                battery: None,
                reversed_ports: Vec::new(),
            });
        }

        // Fetch battery info and active reverse ports for connected devices
        for dev in &mut devices {
            if dev.state == "device" {
                if let Ok(battery_out) = self.run_command(&["-s", &dev.serial, "shell", "dumpsys", "battery"]) {
                    for line in battery_out.lines() {
                        let line = line.trim();
                        if let Some(level) = line.strip_prefix("level:") {
                            dev.battery = Some(format!("{}%", level.trim()));
                            break;
                        }
                    }
                }

                if let Ok(rev_list) = self.reverse_list(&dev.serial) {
                    dev.reversed_ports = rev_list;
                }
            }
        }

        Ok(devices)
    }

    pub fn connect(&self, address: &str) -> Result<String, String> {
        self.run_command(&["connect", address])
    }

    pub fn disconnect(&self, address: &str) -> Result<String, String> {
        self.run_command(&["disconnect", address])
    }

    pub fn disconnect_all(&self) -> Result<String, String> {
        self.run_command(&["disconnect"])
    }

    pub fn pair(&self, address: &str, code: &str) -> Result<String, String> {
        self.run_command(&["pair", address, code])
    }

    pub fn tcpip(&self, port: u16) -> Result<String, String> {
        self.run_command(&["tcpip", &port.to_string()])
    }

    pub fn kill_server(&self) -> Result<String, String> {
        self.run_command(&["kill-server"])
    }

    pub fn start_server(&self) -> Result<String, String> {
        self.run_command(&["start-server"])
    }

    pub fn reboot(&self, serial: &str, mode: Option<&str>) -> Result<String, String> {
        match mode {
            Some(m) if !m.is_empty() => self.run_command(&["-s", serial, "reboot", m]),
            _ => self.run_command(&["-s", serial, "reboot"]),
        }
    }

    pub fn install_apk(&self, serial: &str, apk_path: &Path) -> Result<String, String> {
        let path_str = apk_path
            .to_str()
            .ok_or_else(|| "Invalid APK file path".to_string())?;
        self.run_command(&["-s", serial, "install", "-r", path_str])
    }

    pub fn take_screenshot(&self, serial: &str, save_path: &Path) -> Result<String, String> {
        let adb_path = self
            .resolve_adb_path()
            .ok_or_else(|| "ADB binary not found".to_string())?;

        let mut cmd = Command::new(&adb_path);
        cmd.args(["-s", serial, "exec-out", "screencap", "-p"]);

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let output = cmd.output().map_err(|e| format!("Failed to take screenshot: {}", e))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        std::fs::write(save_path, output.stdout)
            .map_err(|e| format!("Failed to save screenshot file: {}", e))?;

        Ok(format!("Screenshot saved to {:?}", save_path))
    }

    pub fn launch_scrcpy(&self, serial: &str) -> Result<String, String> {
        let scrcpy_path = self
            .resolve_scrcpy_path()
            .ok_or_else(|| "scrcpy not found. Please install scrcpy and add it to PATH, or specify path in Settings.".to_string())?;

        let mut cmd = Command::new(&scrcpy_path);
        cmd.args(["-s", serial]);

        cmd.spawn()
            .map_err(|e| format!("Failed to launch scrcpy: {}", e))?;

        Ok(format!("Launched scrcpy for {}", serial))
    }

    pub fn reverse(&self, serial: &str, device_port: u16, host_port: u16) -> Result<String, String> {
        let dev = format!("tcp:{}", device_port);
        let host = format!("tcp:{}", host_port);
        self.run_command(&["-s", serial, "reverse", &dev, &host])
    }

    pub fn reverse_remove(&self, serial: &str, spec: &str) -> Result<String, String> {
        self.run_command(&["-s", serial, "reverse", "--remove", spec])
    }

    pub fn reverse_remove_all(&self, serial: &str) -> Result<String, String> {
        self.run_command(&["-s", serial, "reverse", "--remove-all"])
    }

    pub fn reverse_list(&self, serial: &str) -> Result<Vec<(String, String)>, String> {
        let out = self.run_command(&["-s", serial, "reverse", "--list"])?;
        let mut list = Vec::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let (d, h) = if parts[0] == "(reverse)" && parts.len() >= 3 {
                    (parts[1].to_string(), parts[2].to_string())
                } else {
                    (parts[0].to_string(), parts[1].to_string())
                };
                list.push((d, h));
            }
        }
        Ok(list)
    }

    pub fn list_mdns_services(&self) -> Result<Vec<MdnsService>, String> {
        let out = self.run_command(&["mdns", "services"])?;
        let mut services = Vec::new();

        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of discovered mdns services") || line.starts_with('*') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let addr_str = parts[parts.len() - 1];
                let service_type = parts[parts.len() - 2].to_string();
                let raw_name = parts[..parts.len() - 2].join(" ");
                let name = clean_service_name(&raw_name);

                if let Some((ip, port_str)) = addr_str.rsplit_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        services.push(MdnsService {
                            name,
                            service_type,
                            ip: ip.to_string(),
                            port,
                        });
                    }
                }
            }
        }

        // Fast TCP probe (200ms) to filter out stale/closed ports left in ADB's mDNS cache
        let mut active_services = Vec::new();
        for s in &services {
            let addr_str = format!("{}:{}", s.ip, s.port);
            let is_alive = if let Ok(sock) = addr_str.parse::<std::net::SocketAddr>() {
                std::net::TcpStream::connect_timeout(&sock, std::time::Duration::from_millis(200)).is_ok()
            } else {
                false
            };

            if is_alive {
                active_services.push(s.clone());
            }
        }

        // If probing verified active services, use them; otherwise fall back to raw list
        let pool = if !active_services.is_empty() {
            active_services
        } else {
            services
        };

        // Deduplicate: if multiple entries exist for the same device (ip & clean name),
        // keep the newest one (last in the list)
        let mut deduped: Vec<MdnsService> = Vec::new();
        for s in pool {
            if let Some(pos) = deduped.iter().position(|existing| existing.ip == s.ip && existing.name == s.name) {
                deduped[pos] = s;
            } else {
                deduped.push(s);
            }
        }

        Ok(deduped)
    }

    pub fn pair_and_auto_connect(
        &self,
        pair_address: &str,
        pair_code: &str,
        explicit_connect_port: Option<u16>,
    ) -> Result<String, String> {
        let pair_out = self.pair(pair_address, pair_code)?;

        // Extract IP from address string (supports "192.168.1.5:12345" or "192.168.1.5")
        let ip = match pair_address.split_once(':') {
            Some((ip_part, _)) => ip_part.trim(),
            None => pair_address.trim(),
        };

        if let Some(port) = explicit_connect_port {
            let target = format!("{}:{}", ip, port);
            match self.connect(&target) {
                Ok(conn_out) => {
                    return Ok(format!("Paired successfully! {}\nConnected: {}", pair_out, conn_out));
                }
                Err(conn_err) => {
                    return Ok(format!(
                        "Paired successfully ({}), but connect to {} failed: {}",
                        pair_out, target, conn_err
                    ));
                }
            }
        }

        // Try discovering connection port via mDNS (Android 11+ TLS connect service)
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if let Ok(services) = self.list_mdns_services() {
            if let Some(svc) = services.iter().find(|s| s.ip == ip && s.service_type.contains("_adb-tls-connect")) {
                let target = format!("{}:{}", svc.ip, svc.port);
                match self.connect(&target) {
                    Ok(_) => {
                        return Ok(format!(
                            "Paired successfully! Automatically connected to {} via mDNS ({}).",
                            target, svc.name
                        ));
                    }
                    Err(err) => {
                        return Ok(format!(
                            "Paired successfully! Found {} via mDNS, but connect failed: {}",
                            target, err
                        ));
                    }
                }
            }
        }

        Ok(format!(
            "Paired successfully with {}!\nNote: Android 11+ uses a separate port for connecting. Look at your phone's main 'Wireless debugging' screen (under 'IP address & Port') and connect to that port in Quick Connect.",
            pair_address
        ))
    }
}
