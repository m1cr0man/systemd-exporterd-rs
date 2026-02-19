use crate::stats::{CPUStats, IOStats, IPStats, MemoryStats, TaskStats, UnitStatus};
use zbus_systemd::{
    systemd1::{ServiceProxy, UnitProxy},
    zbus::Result,
};

use super::systemd_stats;

pub struct Unit<'u> {
    pub name: String,
    pub machine: String,
    pub identifier: String,
    pub restarted: bool,

    pub unit_status: UnitStatus,
    pub task_stats: TaskStats,

    pub collect_io: bool,
    pub io_stats: Option<IOStats>,
    pub collect_ip: bool,
    pub ip_stats: Option<IPStats>,
    pub collect_cpu: bool,
    pub cpu_stats: Option<CPUStats>,
    pub collect_mem: bool,
    pub mem_stats: Option<MemoryStats>,

    pub(super) unit_proxy: UnitProxy<'u>,
    pub(super) service_proxy: Option<ServiceProxy<'u>>,
}

impl<'u> Unit<'u> {
    pub(super) async fn build(
        name: String,
        unit_proxy: UnitProxy<'u>,
        service_proxy: Option<ServiceProxy<'u>>,
    ) -> Result<Self> {
        let id = format!("{}@{}", name, "localhost");
        let mut collect_io = false;
        let mut collect_ip = false;
        let mut collect_cpu = false;
        let mut collect_mem = false;
        if let Some(proxy) = service_proxy.as_ref() {
            collect_io = proxy.io_accounting().await?;
            collect_ip = proxy.ip_accounting().await?;
            collect_cpu = proxy.cpu_accounting().await?;
            collect_mem = proxy.memory_accounting().await?;
        }
        Ok(Self {
            name,
            machine: "localhost".to_string(),
            identifier: id,
            restarted: false,
            unit_status: UnitStatus::default(),
            task_stats: TaskStats::default(),
            collect_io,
            io_stats: None,
            collect_ip,
            ip_stats: None,
            collect_cpu,
            cpu_stats: None,
            collect_mem,
            mem_stats: None,
            unit_proxy,
            service_proxy,
        })
    }

    pub(super) async fn collect_unit_status<'a>(mut self) -> Result<Self> {
        let proxy = &self.unit_proxy;
        let status = systemd_stats::read_unit_status(proxy).await?;
let last_ts = self.unit_status.active_ts;
        self.restarted = last_ts > 0 && last_ts != status.active_ts;
        self.unit_status = status;
        Ok(self)
    }

    pub(super) async fn collect_service_stats<'a>(mut self) -> Result<Self> {
        let proxy = match &self.service_proxy {
            Some(p) => p,
            None => return Ok(self),
        };

        self.task_stats = systemd_stats::read_task_stats(proxy).await?;

        if self.collect_io {
            self.io_stats = Some(systemd_stats::read_io_stats(proxy).await?);
        }

        if self.collect_ip {
            self.ip_stats = Some(systemd_stats::read_ip_stats(proxy).await?);
        }

        if self.collect_cpu {
            self.cpu_stats = Some(systemd_stats::read_cpu_stats(proxy).await?);
        }

        if self.collect_mem {
            self.mem_stats = Some(systemd_stats::read_memory_stats(proxy).await?);
        }

        Ok(self)
    }

    pub async fn collect_stats(self) -> Result<Self> {
        self.collect_unit_status()
            .await?
            .collect_service_stats()
            .await
    }

    pub fn is_service(&self) -> bool {
        self.service_proxy.is_some()
    }
}
