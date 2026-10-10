use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub ip: String,
    pub port: u16,
}

pub struct SubnetScanner {
    is_scanning: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
}

impl SubnetScanner {
    pub fn new() -> Self {
        Self {
            is_scanning: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.is_scanning.load(Ordering::SeqCst)
    }

    pub fn get_progress(&self) -> usize {
        self.progress.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_scanning.store(false, Ordering::SeqCst);
    }

    pub fn get_local_ip() -> Option<String> {
        local_ip_address::local_ip().ok().map(|ip| ip.to_string())
    }

    pub fn get_subnet_prefix() -> Option<String> {
        if let Some(ip_str) = Self::get_local_ip() {
            let parts: Vec<&str> = ip_str.split('.').collect();
            if parts.len() == 4 {
                return Some(format!("{}.{}.{}.", parts[0], parts[1], parts[2]));
            }
        }
        None
    }

    pub fn start_scan(
        &self,
        subnet_prefix: String,
        port: u16,
        result_sender: Sender<ScanResult>,
    ) {
        if self.is_scanning() {
            return;
        }

        self.is_scanning.store(true, Ordering::SeqCst);
        self.progress.store(0, Ordering::SeqCst);

        let is_scanning = Arc::clone(&self.is_scanning);
        let progress = Arc::clone(&self.progress);

        thread::spawn(move || {
            let total = 254;
            let chunk_size = 16;
            let hosts: Vec<u8> = (1..=total as u8).collect();

            for chunk in hosts.chunks(chunk_size) {
                if !is_scanning.load(Ordering::SeqCst) {
                    break;
                }

                let mut handles = vec![];
                for &host_num in chunk {
                    let target_ip = format!("{}{}", subnet_prefix, host_num);
                    let sender_clone = result_sender.clone();
                    let is_scanning_clone = Arc::clone(&is_scanning);

                    let handle = thread::spawn(move || {
                        if !is_scanning_clone.load(Ordering::SeqCst) {
                            return;
                        }

                        let addr_str = format!("{}:{}", target_ip, port);
                        if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                            if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
                                let _ = sender_clone.send(ScanResult {
                                    ip: target_ip,
                                    port,
                                });
                            }
                        }
                    });
                    handles.push(handle);
                }

                for h in handles {
                    let _ = h.join();
                    progress.fetch_add(1, Ordering::SeqCst);
                }
            }

            is_scanning.store(false, Ordering::SeqCst);
        });
    }
}
