use zbus_systemd::{
    systemd1::{ServiceProxy, UnitProxy},
    zbus::Result,
};

#[derive(Default)]
pub struct IOStats {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub read_ops: u64,
    pub write_ops: u64,
}

#[derive(Default)]
pub struct IPStats {
    pub egress_bytes: u64,
    pub ingress_bytes: u64,
    pub egress_packets: u64,
    pub ingress_packets: u64,
}

#[derive(Default)]
pub struct CPUStats {
    pub usage_nsec: u64,
}

#[derive(Default)]
pub struct MemoryStats {
    pub current: u64,
    pub available: u64,
    pub peak: u64,
    pub swap: u64,
    pub swap_peak: u64,
}

#[derive(Default)]
pub struct TaskStats {
    pub count: u64,
}

pub struct Unit<'u> {
    pub name: String,
    pub machine: String,
    pub identifier: String,

    pub active_state: String,
    pub sub_state: String,
    pub active_ts: u64,
    pub inactive_ts: u64,
    pub start_ts: u64,
    pub stop_ts: u64,
    pub restarted: bool,

    pub main_pid: u32,
    pub collect_io: bool,
    pub io_stats: Option<IOStats>,
    pub collect_ip: bool,
    pub ip_stats: Option<IPStats>,
    pub collect_cpu: bool,
    pub cpu_stats: Option<CPUStats>,
    pub collect_mem: bool,
    pub mem_stats: Option<MemoryStats>,
    pub collect_tasks: bool,
    pub tasks_stats: Option<TaskStats>,

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
        let mut collect_tasks = false;
        if let Some(proxy) = service_proxy.as_ref() {
            collect_io = proxy.io_accounting().await?;
            collect_ip = proxy.ip_accounting().await?;
            collect_cpu = proxy.cpu_accounting().await?;
            collect_mem = proxy.memory_accounting().await?;
            collect_tasks = proxy.tasks_accounting().await?;
        }
        Ok(Self {
            name,
            collect_io,
            collect_ip,
            collect_cpu,
            collect_mem,
            collect_tasks,
            machine: "localhost".to_string(),
            identifier: id,
            restarted: false,
            io_stats: None,
            ip_stats: None,
            cpu_stats: None,
            mem_stats: None,
            tasks_stats: None,
            active_state: String::default(),
            sub_state: String::default(),
            main_pid: u32::default(),
            active_ts: u64::default(),
            inactive_ts: u64::default(),
            start_ts: u64::default(),
            stop_ts: u64::default(),
            unit_proxy,
            service_proxy,
        })
    }

    pub(super) async fn collect_unit_stats<'a>(mut self) -> Result<Self> {
        let proxy = self.unit_proxy;
        self.active_state = proxy.active_state().await?;
        self.sub_state = proxy.sub_state().await?;
        self.inactive_ts = proxy.inactive_enter_timestamp().await?;

        let new_active_ts = proxy.active_enter_timestamp().await?;
        self.restarted = self.active_ts > 0 && self.active_ts != new_active_ts;
        self.active_ts = new_active_ts;

        self.unit_proxy = proxy;

        Ok(self)
    }

    pub(super) async fn collect_service_stats<'a>(mut self) -> Result<Self> {
        let proxy = match self.service_proxy {
            Some(p) => p,
            None => return Ok(self),
        };
        if self.collect_io {
            self.io_stats = Some(IOStats {
                read_bytes: proxy.io_read_bytes().await?,
                write_bytes: proxy.io_write_bytes().await?,
                read_ops: proxy.io_read_operations().await?,
                write_ops: proxy.io_write_operations().await?,
            });
        }

        if self.collect_ip {
            self.ip_stats = Some(IPStats {
                egress_bytes: proxy.ip_egress_bytes().await?,
                ingress_bytes: proxy.ip_ingress_bytes().await?,
                egress_packets: proxy.ip_egress_packets().await?,
                ingress_packets: proxy.ip_ingress_packets().await?,
            });
        }

        if self.collect_cpu {
            self.cpu_stats = Some(CPUStats {
                usage_nsec: proxy.cpu_usage_n_sec().await?,
            });
        }

        if self.collect_mem {
            self.mem_stats = Some(MemoryStats {
                current: proxy.memory_current().await?,
                available: proxy.memory_available().await?,
                peak: proxy.memory_peak().await?,
                swap: proxy.memory_swap_current().await?,
                swap_peak: proxy.memory_swap_peak().await?,
            });
        }

        if self.collect_tasks {
            self.tasks_stats = Some(TaskStats {
                count: proxy.tasks_current().await?,
            });
        }

        self.main_pid = proxy.main_pid().await?;

        self.start_ts = proxy.exec_main_start_timestamp().await?;
        self.stop_ts = proxy.exec_main_exit_timestamp().await?;

        self.service_proxy = Some(proxy);

        Ok(self)
    }

    pub async fn collect_stats(self) -> Result<Self> {
        self.collect_unit_stats()
            .await?
            .collect_service_stats()
            .await
    }

    pub fn is_service(&self) -> bool {
        self.service_proxy.is_some()
    }
}
