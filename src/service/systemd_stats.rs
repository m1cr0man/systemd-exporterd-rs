use zbus_systemd::{
    systemd1::{ServiceProxy, UnitProxy},
    zbus::Result,
};

use crate::stats::{CPUStats, IOStats, IPStats, MemoryStats, TaskStats, UnitStatus};

pub(super) async fn read_io_stats(proxy: &ServiceProxy<'_>) -> Result<IOStats> {
    Ok(IOStats {
        read_bytes: proxy.io_read_bytes().await?,
        write_bytes: proxy.io_write_bytes().await?,
        read_ops: proxy.io_read_operations().await?,
        write_ops: proxy.io_write_operations().await?,
    })
}

pub(super) async fn read_ip_stats(proxy: &ServiceProxy<'_>) -> Result<IPStats> {
    Ok(IPStats {
        egress_bytes: proxy.ip_egress_bytes().await?,
        ingress_bytes: proxy.ip_ingress_bytes().await?,
        egress_packets: proxy.ip_egress_packets().await?,
        ingress_packets: proxy.ip_ingress_packets().await?,
    })
}

pub(super) async fn read_cpu_stats(proxy: &ServiceProxy<'_>) -> Result<CPUStats> {
    Ok(CPUStats {
        usage_nsec: proxy.cpu_usage_n_sec().await?,
    })
}

pub(super) async fn read_memory_stats(proxy: &ServiceProxy<'_>) -> Result<MemoryStats> {
    Ok(MemoryStats {
        current: proxy.memory_current().await?,
        available: proxy.memory_available().await?,
        peak: proxy.memory_peak().await?,
        swap: proxy.memory_swap_current().await?,
        swap_peak: proxy.memory_swap_peak().await?,
    })
}

pub(super) async fn read_task_stats(proxy: &ServiceProxy<'_>) -> Result<TaskStats> {
    Ok(TaskStats {
        count: proxy.tasks_current().await?,
        main_pid: proxy.main_pid().await?,
        start_ts: proxy.exec_main_start_timestamp().await?,
        stop_ts: proxy.exec_main_exit_timestamp().await?,
    })
}

pub(super) async fn read_unit_status(proxy: &UnitProxy<'_>) -> Result<UnitStatus> {
    Ok(UnitStatus {
        active_state: proxy.active_state().await?,
        sub_state: proxy.sub_state().await?,
        active_ts: proxy.active_enter_timestamp().await?,
        inactive_ts: proxy.inactive_enter_timestamp().await?,
    })
}
