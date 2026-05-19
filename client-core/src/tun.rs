use std::net::Ipv4Addr;
use std::sync::Arc;
use wintun::{Adapter, Session};

/// WinTun 虚拟网卡封装
pub struct TunDevice {
    pub session: Arc<Session>,
    _adapter: Arc<Adapter>,
}

impl TunDevice {
    /// 创建虚拟网卡并配置 IP 地址
    pub fn create(virtual_ip: Ipv4Addr, subnet_mask: Ipv4Addr) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // 加载 wintun.dll
        let wintun = unsafe { wintun::load()? };

        // 创建虚拟网卡适配器（返回 Arc<Adapter>）
        let adapter = Adapter::create(&wintun, "VLanGaming", "VLanGaming Tunnel", None)?;

        // 使用 netsh 配置 IP 地址
        let ip_str = virtual_ip.to_string();
        let mask_str = subnet_mask.to_string();
        let status = std::process::Command::new("netsh")
            .args([
                "interface", "ip", "set", "address",
                "name=VLanGaming",
                "source=static",
                &format!("addr={}", ip_str),
                &format!("mask={}", mask_str),
                "gateway=none",
            ])
            .output()?;

        if !status.status.success() {
            let stderr = String::from_utf8_lossy(&status.stderr);
            return Err(format!("配置 IP 失败: {}", stderr).into());
        }

        // 启动会话（ring buffer 大小 0x200000 = 2MB）
        let session = adapter.start_session(0x200000)?;

        Ok(TunDevice {
            session: Arc::new(session),
            _adapter: adapter,
        })
    }

    /// 从虚拟网卡读取一个 IP 包（阻塞）
    pub fn read_packet(&self) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let packet = self.session.receive_blocking()?;
        Ok(packet.bytes().to_vec())
    }

    /// 向虚拟网卡写入一个 IP 包
    pub fn write_packet(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_pack = self.session.allocate_send_packet(data.len() as u16)?;
        write_pack.bytes_mut().copy_from_slice(data);
        self.session.send_packet(write_pack);
        Ok(())
    }
}
