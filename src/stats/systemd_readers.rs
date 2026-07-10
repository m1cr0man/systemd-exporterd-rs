use enum_dispatch::enum_dispatch;
use zbus_systemd::{
    systemd1::{MountProxy, ScopeProxy, ServiceProxy, SliceProxy, SocketProxy, UnitProxy},
    zbus::Result,
};

use super::{AccountingFlags, CPUStats, IOStats, IPStats, MemoryStats, TaskStats, UnitStatus};

#[enum_dispatch]
pub trait ResourceStatsReader {
    async fn read_io_stats(&self) -> Result<IOStats>;
    async fn read_cpu_stats(&self) -> Result<CPUStats>;
    async fn read_memory_stats(&self) -> Result<MemoryStats>;
    async fn read_ip_stats(&self) -> Result<IPStats>;
    async fn read_accounting_flags(&self) -> Result<AccountingFlags>;
}

#[enum_dispatch]
pub trait TaskStatsReader {
    async fn read_task_stats(&self) -> Result<TaskStats>;
}

macro_rules! impl_resource_stats_reader {
    ($($proxy:ty),+) => {
        $(impl ResourceStatsReader for $proxy {
            async fn read_io_stats(&self) -> Result<IOStats> {
                Ok(IOStats {
                    read_bytes: self.io_read_bytes().await?,
                    write_bytes: self.io_write_bytes().await?,
                    read_ops: self.io_read_operations().await?,
                    write_ops: self.io_write_operations().await?,
                })
            }
            async fn read_cpu_stats(&self) -> Result<CPUStats> {
                Ok(CPUStats {
                    usage_nsec: self.cpu_usage_n_sec().await?,
                })
            }
            async fn read_memory_stats(&self) -> Result<MemoryStats> {
                Ok(MemoryStats {
                    current: self.memory_current().await?,
                    available: self.memory_available().await?,
                    peak: self.memory_peak().await?,
                    swap: self.memory_swap_current().await?,
                    swap_peak: self.memory_swap_peak().await?,
                })
            }
            async fn read_ip_stats(&self) -> Result<IPStats> {
                Ok(IPStats {
                    egress_bytes: self.ip_egress_bytes().await?,
                    ingress_bytes: self.ip_ingress_bytes().await?,
                    egress_packets: self.ip_egress_packets().await?,
                    ingress_packets: self.ip_ingress_packets().await?,
                })
            }
            async fn read_accounting_flags(&self) -> Result<AccountingFlags> {
                Ok(AccountingFlags {
                    cpu: self.cpu_accounting().await?,
                    memory: self.memory_accounting().await?,
                    io: self.io_accounting().await?,
                    ip: self.ip_accounting().await?,
                    tasks: self.tasks_accounting().await?,
                })
            }
        })+
    };
}

macro_rules! impl_task_stats_reader {
    ($($proxy:ty),+) => {
        $(impl TaskStatsReader for $proxy {
            async fn read_task_stats(&self) -> Result<TaskStats> {
                Ok(TaskStats {
                    count: self.tasks_current().await?,
                    main_pid: self.main_pid().await?,
                    start_ts: self.exec_main_start_timestamp().await?,
                    stop_ts: self.exec_main_exit_timestamp().await?,
                })
            }
        })+
    };
}

impl_resource_stats_reader!(
    MountProxy<'_>,
    ServiceProxy<'_>,
    SliceProxy<'_>,
    SocketProxy<'_>,
    ScopeProxy<'_>
);
impl_task_stats_reader!(ServiceProxy<'_>);

#[enum_dispatch(ResourceStatsReader)]
pub(crate) enum ResourceStatsProxy<'a> {
    Mount(MountProxy<'a>),
    Service(ServiceProxy<'a>),
    Slice(SliceProxy<'a>),
    Socket(SocketProxy<'a>),
    Scope(ScopeProxy<'a>),
}

#[enum_dispatch(TaskStatsReader)]
pub(crate) enum TaskStatsProxy<'a> {
    Service(ServiceProxy<'a>),
}

// To keep the pattern somewhat the same.
// There will only ever be the UnitProxy which implements these methods.
pub trait UnitStatusReader {
    async fn read_status(&self) -> Result<UnitStatus>;
}

impl UnitStatusReader for UnitProxy<'_> {
    async fn read_status(&self) -> Result<UnitStatus> {
        Ok(UnitStatus {
            job_id: self.job().await?.0,
            active_state: self.active_state().await?,
            sub_state: self.sub_state().await?,
            active_ts: self.active_enter_timestamp().await?,
            inactive_ts: self.inactive_enter_timestamp().await?,
        })
    }
}
