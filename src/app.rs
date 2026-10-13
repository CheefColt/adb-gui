use crate::adb::{AdbManager, DeviceInfo, MdnsService};
use crate::scanner::{ScanResult, SubnetScanner};
use crate::storage::AppConfig;
use chrono::Local;
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTab {
    Connect,
    Devices,
    Scanner,
    Settings,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub command: String,
    pub is_success: bool,
    pub message: String,
}

pub enum AppMessage {
    DeviceListUpdated(Result<Vec<DeviceInfo>, String>),
    CommandFinished {
        cmd_name: String,
        result: Result<String, String>,
    },
    AdbVersionDetected(Result<String, String>),
    ScanFound(ScanResult),
    ScanFinished,
    MdnsServicesUpdated(Result<Vec<MdnsService>, String>),
}

pub struct AdbApp {
    config: AppConfig,
    adb_manager: AdbManager,
    scanner: SubnetScanner,

    // Channels
    msg_sender: Sender<AppMessage>,
    msg_receiver: Receiver<AppMessage>,

    // App State
    active_tab: ActiveTab,
    devices: Vec<DeviceInfo>,
    is_refreshing_devices: bool,
    adb_version_info: String,
    is_adb_available: bool,

    // Wireless Connect Tab Inputs
    connect_ip: String,
    connect_port: String,
    connect_nickname: String,
    is_connecting: bool,

    // Android 11+ Pairing Inputs
    pair_address: String,
    pair_code: String,
    pair_connect_port: String,
    is_pairing: bool,

    // mDNS Discovery State
    mdns_services: Vec<MdnsService>,
    is_refreshing_mdns: bool,

    // Scanner Tab State
    subnet_prefix: String,
    scan_port: String,
    discovered_devices: Vec<ScanResult>,

    // Settings & Custom Command
    custom_adb_input: String,
    custom_scrcpy_input: String,
    custom_cmd_input: String,

    // Reverse Port State
    reverse_device_port: String,
    reverse_host_port: String,

    // Logs & Status
    logs: Vec<LogEntry>,
    status_toast: Option<(String, bool, Instant)>,
    last_auto_refresh: Instant,
}

impl AdbApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure dark theme with smooth fonts
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let config = AppConfig::load();
        let adb_manager = AdbManager::new(
            config.custom_adb_path.clone(),
            config.custom_scrcpy_path.clone(),
        );
        let scanner = SubnetScanner::new();

        let (msg_sender, msg_receiver) = channel();

        let default_subnet = SubnetScanner::get_subnet_prefix().unwrap_or_else(|| "192.168.1.".to_string());

        let mut app = Self {
            config: config.clone(),
            adb_manager: adb_manager.clone(),
            scanner,
            msg_sender: msg_sender.clone(),
            msg_receiver,
            active_tab: ActiveTab::Connect,
            devices: Vec::new(),
            is_refreshing_devices: false,
            adb_version_info: "Detecting ADB...".to_string(),
            is_adb_available: false,
            connect_ip: "192.168.1.".to_string(),
            connect_port: config.default_port.to_string(),
            connect_nickname: String::new(),
            is_connecting: false,
            pair_address: "192.168.1.:".to_string(),
            pair_code: String::new(),
            pair_connect_port: String::new(),
            is_pairing: false,
            mdns_services: Vec::new(),
            is_refreshing_mdns: false,
            subnet_prefix: default_subnet,
            scan_port: "5555".to_string(),
            discovered_devices: Vec::new(),
            custom_adb_input: config.custom_adb_path.clone().unwrap_or_default(),
            custom_scrcpy_input: config.custom_scrcpy_path.clone().unwrap_or_default(),
            custom_cmd_input: "shell getprop ro.product.model".to_string(),
            reverse_device_port: "8081".to_string(),
            reverse_host_port: "8081".to_string(),
            logs: Vec::new(),
            status_toast: None,
            last_auto_refresh: Instant::now(),
        };

        // Detect ADB on start
        app.check_adb_version();
        app.refresh_devices();
        app.refresh_mdns();

        app
    }

    fn add_log(&mut self, command: &str, is_success: bool, message: &str) {
        let entry = LogEntry {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            command: command.to_string(),
            is_success,
            message: message.to_string(),
        };
        self.logs.push(entry);
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    fn set_toast(&mut self, message: &str, is_success: bool) {
        self.status_toast = Some((message.to_string(), is_success, Instant::now()));
    }

    fn check_adb_version(&mut self) {
        let manager = self.adb_manager.clone();
        let sender = self.msg_sender.clone();
        thread::spawn(move || {
            let res = manager.get_adb_version();
            let _ = sender.send(AppMessage::AdbVersionDetected(res));
        });
    }

    pub fn refresh_devices(&mut self) {
        if self.is_refreshing_devices {
            return;
        }
        self.is_refreshing_devices = true;
        let manager = self.adb_manager.clone();
        let sender = self.msg_sender.clone();
        thread::spawn(move || {
            let res = manager.list_devices();
            let _ = sender.send(AppMessage::DeviceListUpdated(res));
        });
    }

    pub fn refresh_mdns(&mut self) {
        if self.is_refreshing_mdns {
            return;
        }
        self.is_refreshing_mdns = true;
        let manager = self.adb_manager.clone();
        let sender = self.msg_sender.clone();
        thread::spawn(move || {
            let res = manager.list_mdns_services();
            let _ = sender.send(AppMessage::MdnsServicesUpdated(res));
        });
    }

    pub fn validate_ip_port(addr: &str) -> Result<(), String> {
        let (ip_str, port_str) = addr
            .split_once(':')
            .ok_or_else(|| "Missing port. Format must be IP:PORT (e.g. 192.168.1.50:37123)".to_string())?;

        let ip_trimmed = ip_str.trim();
        let port_trimmed = port_str.trim();

        if ip_trimmed.parse::<std::net::Ipv4Addr>().is_err() {
            return Err(format!(
                "Invalid IPv4 address '{}'. Expected standard 4-octet format (e.g. 192.168.29.49)",
                ip_trimmed
            ));
        }

        if port_trimmed.parse::<u16>().is_err() {
            return Err(format!(
                "Invalid port number '{}'. Must be between 1 and 65535",
                port_trimmed
            ));
        }

        Ok(())
    }

    fn run_async_command<F>(&self, name: &'static str, f: F)
    where
        F: FnOnce(&AdbManager) -> Result<String, String> + Send + 'static,
    {
        let manager = self.adb_manager.clone();
        let sender = self.msg_sender.clone();
        thread::spawn(move || {
            let res = f(&manager);
            let _ = sender.send(AppMessage::CommandFinished {
                cmd_name: name.to_string(),
                result: res,
            });
        });
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.msg_receiver.try_recv() {
            match msg {
                AppMessage::DeviceListUpdated(res) => {
                    self.is_refreshing_devices = false;
                    match res {
                        Ok(list) => {
                            self.devices = list;
                        }
                        Err(err) => {
                            self.add_log("list devices", false, &err);
                        }
                    }
                }
                AppMessage::MdnsServicesUpdated(res) => {
                    self.is_refreshing_mdns = false;
                    if let Ok(list) = res {
                        self.mdns_services = list;
                    }
                }
                AppMessage::CommandFinished { cmd_name, result } => {
                    match result {
                        Ok(out) => {
                            let msg = if out.trim().is_empty() {
                                "Command completed successfully".to_string()
                            } else {
                                out
                            };
                            self.add_log(&cmd_name, true, &msg);
                            self.set_toast(&format!("{}: Success", cmd_name), true);
                            self.refresh_devices();
                            if cmd_name == "Pair & Connect" || cmd_name == "Pair Device" {
                                if let Some((ip, _)) = self.pair_address.split_once(':') {
                                    let clean_ip = ip.trim().to_string();
                                    if !clean_ip.is_empty() {
                                        self.connect_ip = clean_ip;
                                    }
                                }
                                self.refresh_mdns();
                            }
                        }
                        Err(err) => {
                            self.add_log(&cmd_name, false, &err);
                            self.set_toast(&format!("{}: Failed", cmd_name), false);
                        }
                    }
                    self.is_connecting = false;
                    self.is_pairing = false;
                }
                AppMessage::AdbVersionDetected(res) => match res {
                    Ok(ver) => {
                        self.is_adb_available = true;
                        self.adb_version_info = ver;
                    }
                    Err(e) => {
                        self.is_adb_available = false;
                        self.adb_version_info = format!("Not found: {}", e);
                    }
                },
                AppMessage::ScanFound(result) => {
                    if !self.discovered_devices.iter().any(|d| d.ip == result.ip && d.port == result.port) {
                        self.discovered_devices.push(result);
                    }
                }
                AppMessage::ScanFinished => {
                    self.set_toast("Network scan complete", true);
                }
            }
        }
    }
}

impl eframe::App for AdbApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_messages();

        // Auto refresh handling
        if self.config.auto_refresh
            && self.is_adb_available
            && self.last_auto_refresh.elapsed() >= Duration::from_secs(self.config.refresh_interval_secs)
        {
            self.last_auto_refresh = Instant::now();
            self.refresh_devices();
            self.refresh_mdns();
        }

        // Top Panel: App Header & ADB Status
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("⚡ ADB Bridge GUI").strong().color(egui::Color32::from_rgb(99, 179, 237)));
                ui.add_space(15.0);

                if self.is_adb_available {
                    ui.label(egui::RichText::new("● ADB Ready").color(egui::Color32::from_rgb(72, 187, 120)).strong());
                } else {
                    ui.label(egui::RichText::new("● ADB Not Found").color(egui::Color32::from_rgb(245, 101, 101)).strong());
                }

                ui.add_space(10.0);
                ui.label(egui::RichText::new(&self.adb_version_info).weak().size(11.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🛑 Kill Server").clicked() {
                        self.run_async_command("Kill Server", |m| m.kill_server());
                    }
                    if ui.button("⚡ Restart Server").clicked() {
                        self.run_async_command("Restart Server", |m| {
                            let _ = m.kill_server();
                            m.start_server()
                        });
                    }
                    if ui.button(if self.is_refreshing_devices || self.is_refreshing_mdns { "🔄 Refreshing..." } else { "🔄 Refresh" }).clicked() {
                        self.refresh_devices();
                        self.refresh_mdns();
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();

            // Navigation tabs
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, ActiveTab::Connect, egui::RichText::new("📱 Connect & History").size(14.0));
                ui.add_space(8.0);
                let dev_count_text = format!("💻 Connected Devices ({})", self.devices.len());
                ui.selectable_value(&mut self.active_tab, ActiveTab::Devices, egui::RichText::new(dev_count_text).size(14.0));
                ui.add_space(8.0);
                ui.selectable_value(&mut self.active_tab, ActiveTab::Scanner, egui::RichText::new("🔍 Network Scanner").size(14.0));
                ui.add_space(8.0);
                ui.selectable_value(&mut self.active_tab, ActiveTab::Settings, egui::RichText::new("⚙ Settings & Console").size(14.0));
            });
            ui.add_space(4.0);
        });

        // Bottom Panel: Toast notification & Log console
        egui::Panel::bottom("bottom_panel")
            .resizable(true)
            .min_size(90.0)
            .default_size(140.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Console Logs").strong().size(12.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear Logs").clicked() {
                            self.logs.clear();
                        }
                        if let Some((msg, is_success, time)) = &self.status_toast {
                            if time.elapsed() < Duration::from_secs(4) {
                                let color = if *is_success {
                                    egui::Color32::from_rgb(72, 187, 120)
                                } else {
                                    egui::Color32::from_rgb(245, 101, 101)
                                };
                                ui.label(egui::RichText::new(format!("Toast: {}", msg)).color(color));
                            }
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if self.logs.is_empty() {
                            ui.label(egui::RichText::new("No command logs yet. Actions will appear here.").weak().italics());
                        } else {
                            for log in &self.logs {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&log.timestamp).weak().size(11.0));
                                    let badge_color = if log.is_success {
                                        egui::Color32::from_rgb(72, 187, 120)
                                    } else {
                                        egui::Color32::from_rgb(245, 101, 101)
                                    };
                                    let badge_text = if log.is_success { "OK" } else { "ERR" };
                                    ui.label(egui::RichText::new(format!("[{}]", badge_text)).color(badge_color).strong().size(11.0));
                                    ui.label(egui::RichText::new(&log.command).strong().size(11.0));
                                    ui.label(egui::RichText::new(&log.message).size(11.0));
                                });
                            }
                        }
                    });
            });

        // Central Panel: Tab Contents
        egui::CentralPanel::default().show(ui, |ui| {
            match self.active_tab {
                ActiveTab::Connect => self.render_connect_tab(ui),
                ActiveTab::Devices => self.render_devices_tab(ui),
                ActiveTab::Scanner => self.render_scanner_tab(ui),
                ActiveTab::Settings => self.render_settings_tab(ui),
            }
        });

        // Repaint periodically if scanning or refreshing
        if self.scanner.is_scanning() || self.is_refreshing_devices {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
    }
}

impl AdbApp {
    fn render_connect_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            // Row with Quick Connect & USB to Wireless card
            ui.horizontal(|ui| {
                // Column 1: Quick Connect
                ui.vertical(|ui| {
                    ui.set_width(450.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.heading("⚡ Quick Wireless Connect");
                        ui.add_space(4.0);
                        ui.label("Enter your Android device IP address and port (default 5555):");
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.label("IP Address:");
                            ui.text_edit_singleline(&mut self.connect_ip);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Port:         ");
                            ui.text_edit_singleline(&mut self.connect_port);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Nickname: ");
                            ui.add(egui::TextEdit::singleline(&mut self.connect_nickname).hint_text("e.g. Pixel 8 / Living Room TV"));
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let connect_btn = ui.add_sized([120.0, 30.0], egui::Button::new(
                                egui::RichText::new("🔗 Connect").strong().color(egui::Color32::WHITE)
                            ).fill(egui::Color32::from_rgb(49, 130, 206)));

                            if connect_btn.clicked() && !self.is_connecting {
                                let ip = self.connect_ip.trim().to_string();
                                let port: u16 = self.connect_port.trim().parse().unwrap_or(5555);
                                let nick = if self.connect_nickname.trim().is_empty() {
                                    None
                                } else {
                                    Some(self.connect_nickname.trim().to_string())
                                };

                                self.config.add_or_update_device(&ip, port, nick);
                                let target = format!("{}:{}", ip, port);
                                self.is_connecting = true;
                                self.run_async_command("Connect", move |m| m.connect(&target));
                            }

                            if ui.add_sized([110.0, 30.0], egui::Button::new("Disconnect")).clicked() {
                                let ip = self.connect_ip.trim().to_string();
                                let port = self.connect_port.trim();
                                let target = format!("{}:{}", ip, port);
                                self.run_async_command("Disconnect", move |m| m.disconnect(&target));
                            }

                            if ui.add_sized([120.0, 30.0], egui::Button::new("Disconnect All")).clicked() {
                                self.run_async_command("Disconnect All", |m| m.disconnect_all());
                            }
                        });
                    });

                    ui.add_space(10.0);

                    // USB to Wireless Helper Card
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.heading("🔌 Switch USB to Wireless Mode");
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(
                            "Plug your Android device in via USB once, click this button to open port 5555, then unplug the cable and connect wirelessly!"
                        ).weak());
                        ui.add_space(6.0);

                        if ui.add_sized([260.0, 30.0], egui::Button::new("⚡ Enable Wireless on USB (tcpip 5555)")).clicked() {
                            self.run_async_command("Enable TCP/IP 5555", |m| m.tcpip(5555));
                        }
                    });
                });

                ui.add_space(15.0);

                // Column 2: Android 11+ Wireless Pairing & mDNS Discovered Devices
                ui.vertical(|ui| {
                    ui.set_width(450.0);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.heading("📶 Android 11+ Wireless Pairing");
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(
                            "1. On your phone: Settings > Developer options > Wireless debugging\n2. Tap 'Pair device with pairing code'\n3. Enter the Pairing IP:Port and 6-digit Code from the popup dialog.\n   (Connection Port from main screen is optional; auto-discovered via mDNS!)"
                        ).weak().size(12.0));
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label("Pairing IP & Port:   ");
                            ui.add(egui::TextEdit::singleline(&mut self.pair_address).hint_text("192.168.1.xxx:xxxxx"));
                        });

                        ui.horizontal(|ui| {
                            ui.label("6-Digit Code:        ");
                            ui.add(egui::TextEdit::singleline(&mut self.pair_code).hint_text("e.g. 123456"));
                        });

                        ui.horizontal(|ui| {
                            ui.label("Connect Port (opt):");
                            ui.add(egui::TextEdit::singleline(&mut self.pair_connect_port).hint_text("Main screen port (or auto-detect)"));
                        });

                        ui.add_space(8.0);
                        let pair_btn_label = if self.is_pairing { "⏳ Pairing..." } else { "🔐 Pair & Connect" };
                        let pair_btn = ui.add_sized([160.0, 30.0], egui::Button::new(
                            egui::RichText::new(pair_btn_label).strong().color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(56, 161, 105)));

                        if pair_btn.clicked() && !self.is_pairing {
                            let addr = self.pair_address.trim().to_string();
                            let code = self.pair_code.trim().to_string();

                            if addr.is_empty() || code.is_empty() {
                                self.set_toast("Please enter both pairing address and code", false);
                            } else if let Err(err_msg) = Self::validate_ip_port(&addr) {
                                self.set_toast(&err_msg, false);
                                self.add_log("Pair & Connect", false, &err_msg);
                            } else {
                                let explicit_port = self.pair_connect_port.trim().parse::<u16>().ok();
                                self.is_pairing = true;
                                self.run_async_command("Pair & Connect", move |m| {
                                    m.pair_and_auto_connect(&addr, &code, explicit_port)
                                });
                            }
                        }
                    });

                    ui.add_space(10.0);

                    // mDNS Discovered Devices Card
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading("📡 Discovered Wireless Devices (mDNS)");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(if self.is_refreshing_mdns { "🔄 Scanning..." } else { "🔄 Scan mDNS" }).clicked() {
                                    self.refresh_mdns();
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Wireless debugging devices discovered on your local network:").weak().size(12.0));
                        ui.add_space(6.0);

                        let connect_services: Vec<_> = self.mdns_services.iter()
                            .filter(|s| s.service_type.contains("_adb-tls-connect") || s.service_type.contains("_adb."))
                            .cloned()
                            .collect();

                        if connect_services.is_empty() {
                            ui.label(egui::RichText::new("No wireless devices detected via mDNS yet. Make sure Wireless Debugging is enabled and your device is on the same Wi-Fi network.").weak());
                        } else {
                            let mut mdns_to_connect = None;

                            for svc in connect_services {
                                let target = format!("{}:{}", svc.ip, svc.port);
                                let is_connected = self.devices.iter().any(|d| d.serial == target);

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("📶").size(14.0));
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&svc.name).strong().size(12.0));
                                        ui.label(egui::RichText::new(&target).color(egui::Color32::from_rgb(99, 179, 237)).size(11.0));
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if is_connected {
                                            ui.label(egui::RichText::new("● Connected").color(egui::Color32::from_rgb(72, 187, 120)).strong());
                                        } else {
                                            if ui.add_sized([90.0, 24.0], egui::Button::new("🔗 Connect")).clicked() {
                                                mdns_to_connect = Some((svc.ip.clone(), svc.port, svc.name.clone()));
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                            }

                            if let Some((ip, port, name)) = mdns_to_connect {
                                self.connect_ip = ip.clone();
                                self.connect_port = port.to_string();
                                self.config.add_or_update_device(&ip, port, Some(name));
                                let target = format!("{}:{}", ip, port);
                                self.is_connecting = true;
                                self.run_async_command("Connect", move |m| m.connect(&target));
                            }
                        }
                    });
                });
            });

            ui.add_space(15.0);

            // Saved Devices & History Section
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("⭐ Saved & Recent Devices");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Total: {}", self.config.saved_devices.len()));
                    });
                });
                ui.add_space(6.0);

                if self.config.saved_devices.is_empty() {
                    ui.label(egui::RichText::new("No saved devices yet. Connect to a device above to save it here for 1-click reconnecting!").weak());
                } else {
                    let mut to_remove = None;
                    let mut to_connect = None;
                    let mut to_disconnect = None;

                    egui::Grid::new("saved_devices_grid")
                        .striped(true)
                        .min_col_width(120.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Nickname").strong());
                            ui.label(egui::RichText::new("IP Address : Port").strong());
                            ui.label(egui::RichText::new("Last Connected").strong());
                            ui.label(egui::RichText::new("Actions").strong());
                            ui.end_row();

                            for (idx, dev) in self.config.saved_devices.iter().enumerate() {
                                ui.label(&dev.nickname);
                                ui.label(format!("{}:{}", dev.ip, dev.port));
                                ui.label(egui::RichText::new(&dev.last_connected).weak().size(11.0));

                                ui.horizontal(|ui| {
                                    if ui.button("🔗 Connect").clicked() {
                                        to_connect = Some((dev.ip.clone(), dev.port));
                                    }
                                    if ui.button("Disconnect").clicked() {
                                        to_disconnect = Some((dev.ip.clone(), dev.port));
                                    }
                                    if ui.button("🗑 Remove").clicked() {
                                        to_remove = Some(idx);
                                    }
                                });
                                ui.end_row();
                            }
                        });

                    if let Some(idx) = to_remove {
                        self.config.remove_saved_device(idx);
                    }
                    if let Some((ip, port)) = to_connect {
                        self.connect_ip = ip.clone();
                        self.connect_port = port.to_string();
                        let target = format!("{}:{}", ip, port);
                        self.run_async_command("Connect", move |m| m.connect(&target));
                    }
                    if let Some((ip, port)) = to_disconnect {
                        let target = format!("{}:{}", ip, port);
                        self.run_async_command("Disconnect", move |m| m.disconnect(&target));
                    }
                }
            });
        });
    }

    fn render_devices_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.heading("💻 Connected Android Devices");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh List").clicked() {
                        self.refresh_devices();
                    }
                });
            });
            ui.add_space(6.0);

            if self.devices.is_empty() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("No devices currently connected to ADB.").strong());
                    ui.label("Plug in your phone via USB with USB Debugging enabled, or connect via Wi-Fi in the 'Connect' tab.");
                    ui.add_space(10.0);
                });
            } else {
                for dev in self.devices.clone() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Device icon & Name
                            let icon = if dev.is_wireless { "📶" } else { "🔌" };
                            ui.label(egui::RichText::new(format!("{} {}", icon, dev.model)).strong().size(16.0));

                            // State badge
                            let (state_color, state_text) = match dev.state.as_str() {
                                "device" => (egui::Color32::from_rgb(72, 187, 120), "Online"),
                                "unauthorized" => (egui::Color32::from_rgb(237, 137, 54), "Unauthorized (Check screen prompt)"),
                                _ => (egui::Color32::from_rgb(245, 101, 101), dev.state.as_str()),
                            };
                            ui.label(egui::RichText::new(format!("● {}", state_text)).color(state_color).strong());

                            // Battery level if available
                            if let Some(ref bat) = dev.battery {
                                ui.label(egui::RichText::new(format!("🔋 {}", bat)).color(egui::Color32::from_rgb(99, 179, 237)));
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(&dev.serial).weak().size(12.0));
                            });
                        });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Actions toolbar
                        ui.horizontal_wrapped(|ui| {
                            // Mirror Screen button
                            if ui.button("📺 Mirror Screen (scrcpy)").clicked() {
                                let serial = dev.serial.clone();
                                self.run_async_command("Launch scrcpy", move |m| m.launch_scrcpy(&serial));
                            }

                            // Install APK
                            if ui.button("📦 Install APK...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Android Package", &["apk"])
                                    .pick_file()
                                {
                                    let serial = dev.serial.clone();
                                    self.run_async_command("Install APK", move |m| m.install_apk(&serial, &path));
                                }
                            }

                            // Take Screenshot
                            if ui.button("📸 Take Screenshot").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PNG Image", &["png"])
                                    .set_file_name(&format!("screenshot_{}.png", Local::now().format("%Y%m%d_%H%M%S")))
                                    .save_file()
                                {
                                    let serial = dev.serial.clone();
                                    self.run_async_command("Take Screenshot", move |m| m.take_screenshot(&serial, &path));
                                }
                            }

                            // Reboot options
                            ui.menu_button("🔄 Reboot", |ui| {
                                if ui.button("Reboot (Normal)").clicked() {
                                    let s = dev.serial.clone();
                                    self.run_async_command("Reboot", move |m| m.reboot(&s, None));
                                }
                                if ui.button("Reboot to Recovery").clicked() {
                                    let s = dev.serial.clone();
                                    self.run_async_command("Reboot Recovery", move |m| m.reboot(&s, Some("recovery")));
                                }
                                if ui.button("Reboot to Bootloader / Fastboot").clicked() {
                                    let s = dev.serial.clone();
                                    self.run_async_command("Reboot Bootloader", move |m| m.reboot(&s, Some("bootloader")));
                                }
                            });

                            // Disconnect if wireless
                            if dev.is_wireless {
                                if ui.button("Disconnect").clicked() {
                                    let s = dev.serial.clone();
                                    self.run_async_command("Disconnect", move |m| m.disconnect(&s));
                                }
                            }
                        });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Port Reverse section
                        egui::CollapsingHeader::new(egui::RichText::new("🔄 Reverse TCP Ports (Device ➔ PC)").strong())
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Route connections from Android localhost to your PC (e.g. for React Native Metro, Expo, or local APIs):").weak().size(11.0));
                                ui.add_space(4.0);

                                ui.horizontal(|ui| {
                                    ui.label("Device tcp:");
                                    ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(&mut self.reverse_device_port));

                                    ui.label("Host tcp:");
                                    ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(&mut self.reverse_host_port));

                                    let d_port: u16 = self.reverse_device_port.trim().parse().unwrap_or(8081);
                                    let h_port: u16 = self.reverse_host_port.trim().parse().unwrap_or(d_port);
                                    let s_rev = dev.serial.clone();

                                    let rev_btn = ui.add_sized([120.0, 24.0], egui::Button::new(
                                        egui::RichText::new("🔗 Reverse Port").strong().color(egui::Color32::WHITE)
                                    ).fill(egui::Color32::from_rgb(49, 130, 206)));

                                    if rev_btn.clicked() {
                                        self.run_async_command("Reverse TCP", move |m| m.reverse(&s_rev, d_port, h_port));
                                    }

                                    let s_rem_all = dev.serial.clone();
                                    if ui.button("❌ Remove All").clicked() {
                                        self.run_async_command("Reverse Remove All", move |m| m.reverse_remove_all(&s_rem_all));
                                    }
                                });

                                ui.add_space(2.0);
                                // Presets chips
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Presets:").weak().size(11.0));
                                    let presets = [
                                        ("8081 (Metro)", "8081"),
                                        ("3000 (Node/Web)", "3000"),
                                        ("8080 (Server)", "8080"),
                                        ("5173 (Vite)", "5173"),
                                    ];
                                    for (label, port) in presets {
                                        if ui.small_button(label).clicked() {
                                            self.reverse_device_port = port.to_string();
                                            self.reverse_host_port = port.to_string();
                                            let s_preset = dev.serial.clone();
                                            let p: u16 = port.parse().unwrap_or(8081);
                                            self.run_async_command("Reverse TCP", move |m| m.reverse(&s_preset, p, p));
                                        }
                                    }
                                });

                                // Active reverse rules
                                if !dev.reversed_ports.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("Active Reverse Rules:").strong().size(11.0));
                                    for (d_spec, h_spec) in &dev.reversed_ports {
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(format!("  • {} ➔ {}", d_spec, h_spec)).color(egui::Color32::from_rgb(72, 187, 120)).strong());
                                            let s_rem = dev.serial.clone();
                                            let d_clone = d_spec.clone();
                                            if ui.small_button("🗑 Remove").clicked() {
                                                self.run_async_command("Remove Reverse", move |m| m.reverse_remove(&s_rem, &d_clone));
                                            }
                                        });
                                    }
                                }
                            });
                    });
                    ui.add_space(8.0);
                }
            }
        });
    }

    fn render_scanner_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("🔍 Local Subnet Auto-Scanner");
            ui.add_space(4.0);
            ui.label("Scan your Wi-Fi network to automatically discover Android devices listening on port 5555.");
            ui.add_space(8.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Subnet Prefix:");
                    ui.add(egui::TextEdit::singleline(&mut self.subnet_prefix).hint_text("192.168.1."));

                    ui.label("Port:");
                    ui.add_sized([60.0, 20.0], egui::TextEdit::singleline(&mut self.scan_port));

                    if !self.scanner.is_scanning() {
                        if ui.button("🚀 Start Scan").clicked() {
                            self.discovered_devices.clear();
                            let prefix = self.subnet_prefix.clone();
                            let port = self.scan_port.parse().unwrap_or(5555);

                            let sender = self.msg_sender.clone();
                            let (scan_tx, scan_rx) = channel();

                            // Bridge scanner channel into app message channel
                            thread::spawn(move || {
                                while let Ok(item) = scan_rx.recv() {
                                    let _ = sender.send(AppMessage::ScanFound(item));
                                }
                                let _ = sender.send(AppMessage::ScanFinished);
                            });

                            self.scanner.start_scan(prefix, port, scan_tx);
                        }
                    } else {
                        if ui.button("⏹ Stop Scan").clicked() {
                            self.scanner.stop();
                        }
                    }
                });

                if self.scanner.is_scanning() {
                    ui.add_space(6.0);
                    let progress = self.scanner.get_progress() as f32 / 16.0;
                    ui.add(egui::ProgressBar::new(progress).show_percentage().animate(true));
                }
            });

            ui.add_space(10.0);

            // Discovered Devices list
            ui.heading(format!("Discovered Devices ({})", self.discovered_devices.len()));
            ui.add_space(4.0);

            if self.discovered_devices.is_empty() {
                ui.label(egui::RichText::new("No devices found yet. Click 'Start Scan' to search your local network.").weak());
            } else {
                for dev in self.discovered_devices.clone() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("📶 {}:{}", dev.ip, dev.port)).strong());
                        if ui.button("🔗 Connect").clicked() {
                            let target = format!("{}:{}", dev.ip, dev.port);
                            self.config.add_or_update_device(&dev.ip, dev.port, None);
                            self.run_async_command("Connect", move |m| m.connect(&target));
                        }
                    });
                }
            }
        });
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("⚙ Settings & Configuration");
            ui.add_space(8.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Custom Binary Paths").strong());
                ui.add_space(4.0);

                // ADB Path
                ui.horizontal(|ui| {
                    ui.label("ADB Executable:   ");
                    ui.text_edit_singleline(&mut self.custom_adb_input);
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Executable", &["exe"])
                            .pick_file()
                        {
                            self.custom_adb_input = path.to_string_lossy().to_string();
                        }
                    }
                });

                // scrcpy Path
                ui.horizontal(|ui| {
                    ui.label("scrcpy Executable:");
                    ui.text_edit_singleline(&mut self.custom_scrcpy_input);
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Executable", &["exe"])
                            .pick_file()
                        {
                            self.custom_scrcpy_input = path.to_string_lossy().to_string();
                        }
                    }
                });

                ui.add_space(6.0);
                if ui.button("💾 Save Paths").clicked() {
                    self.config.custom_adb_path = if self.custom_adb_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.custom_adb_input.trim().to_string())
                    };
                    self.config.custom_scrcpy_path = if self.custom_scrcpy_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.custom_scrcpy_input.trim().to_string())
                    };
                    let _ = self.config.save();
                    self.adb_manager = AdbManager::new(
                        self.config.custom_adb_path.clone(),
                        self.config.custom_scrcpy_path.clone(),
                    );
                    self.check_adb_version();
                    self.set_toast("Configuration saved", true);
                }
            });

            ui.add_space(10.0);

            // Auto-refresh config
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Preferences").strong());
                ui.add_space(4.0);

                ui.checkbox(&mut self.config.auto_refresh, "Auto-refresh connected devices");

                if self.config.auto_refresh {
                    ui.horizontal(|ui| {
                        ui.label("Refresh interval (seconds):");
                        let mut interval = self.config.refresh_interval_secs as u32;
                        if ui.add(egui::Slider::new(&mut interval, 2..=30)).changed() {
                            self.config.refresh_interval_secs = interval as u64;
                            let _ = self.config.save();
                        }
                    });
                }
            });

            ui.add_space(10.0);

            // Raw ADB Command Console Runner
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Custom ADB Command Runner").strong());
                ui.add_space(4.0);
                ui.label("Execute any adb arguments directly (e.g. 'shell pm list packages -3'):");
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("adb");
                    ui.text_edit_singleline(&mut self.custom_cmd_input);

                    if ui.button("Execute").clicked() {
                        let cmd_str = self.custom_cmd_input.clone();
                        let parts: Vec<String> = cmd_str.split_whitespace().map(|s| s.to_string()).collect();
                        if !parts.is_empty() {
                            let parts_clone = parts.clone();
                            self.run_async_command("Custom Command", move |m| {
                                let slice: Vec<&str> = parts_clone.iter().map(|s| s.as_str()).collect();
                                m.run_command(&slice)
                            });
                        }
                    }
                });
            });
        });
    }
}
