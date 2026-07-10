use crate::cgroup::CGroup;
use crate::stats::{
    AccountingFlags, ResourceStats, TaskStats, UnitStatus,
    systemd_readers::{
        ResourceStatsProxy, ResourceStatsReader, TaskStatsProxy, TaskStatsReader, UnitStatusReader,
    },
};

use zbus_systemd::{systemd1::UnitProxy, zbus::Result};

pub struct Unit<'u> {
    pub name: String,
    pub machine: String,
    pub scope: String,
    pub identifier: String,

    pub status: UnitStatus,
    pub accounting: AccountingFlags,

    pub(crate) unit_proxy: UnitProxy<'u>,
    pub(crate) task_proxy: Option<TaskStatsProxy<'u>>,
    pub(crate) resource_proxy: Option<ResourceStatsProxy<'u>>,
    pub(crate) cgroup: Option<CGroup>,
}

impl<'u> Unit<'u> {
    pub(super) fn new(name: String, scope: String, unit_proxy: UnitProxy<'u>) -> Self {
        let id = format!("{}@{}", name, "localhost");
        Self {
            name,
            machine: "localhost".to_string(),
            scope,
            identifier: id,
            status: UnitStatus::default(),
            accounting: AccountingFlags::default(),
            unit_proxy,
            task_proxy: None,
            resource_proxy: None,
            cgroup: None,
        }
    }

    pub(super) fn with_resource_proxy(mut self, resource_proxy: ResourceStatsProxy<'u>) -> Self {
        self.resource_proxy = Some(resource_proxy);
        self
    }

    pub(super) fn with_task_proxy(mut self, task_proxy: TaskStatsProxy<'u>) -> Self {
        self.task_proxy = Some(task_proxy);
        self
    }

    pub(super) fn with_cgroup(mut self, cgroup: CGroup) -> Self {
        if !cgroup.is_root {
            // We never want to use cgroups pointing to the root of cgroupfs
            self.cgroup = Some(cgroup);
        }
        self
    }

    fn running(&self) -> bool {
        !(self.status.active_state == "inactive" || self.status.sub_state == "exited")
    }

    pub(super) async fn update_unit_status(&mut self) -> Result<()> {
        self.status = self.unit_proxy.read_status().await?;
        if let Some(proxy) = &self.resource_proxy {
            match proxy.read_accounting_flags().await {
                Ok(flags) => self.accounting = flags,
                Err(err) => tracing::warn!(
                    error = %err,
                    name = %self.name,
                    "failed to read accounting flags; keeping previous values",
                ),
            }
        }
        Ok(())
    }

    pub async fn collect_task_stats(&self) -> Result<TaskStats> {
        match &self.task_proxy {
            Some(p) if self.accounting.tasks => p.read_task_stats().await,
            _ => Ok(TaskStats::default()),
        }
    }

    pub async fn collect_resource_stats(&self) -> Result<ResourceStats> {
        let mut stats = ResourceStats::default();

        if !self.running() {
            return Ok(stats);
        }
        let Some(proxy) = &self.resource_proxy else {
            return Ok(stats);
        };

        if self.accounting.ip {
            stats.ip_stats = proxy.read_ip_stats().await?;
        }

        // Reading from systemd dbus is slow due to the deserialisation
        // and validation of bus names. Read from cgroupfs directly where
        // possible, and use systemd as a fallback.
        if self.accounting.io {
            stats.io_stats = match self.cgroup.as_ref() {
                Some(cg) => match cg.read_io_stats().await {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cgroup = %cg.to_string(),
                            name = %self.name,
                            "cgroup io stats read failed; falling back to dbus",
                        );
                        proxy.read_io_stats().await?
                    }
                },
                None => proxy.read_io_stats().await?,
            };
        }
        if self.accounting.cpu {
            stats.cpu_stats = match self.cgroup.as_ref() {
                Some(cg) => match cg.read_cpu_stats().await {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cgroup = %cg.to_string(),
                            name = %self.name,
                            "cgroup cpu stats read failed; falling back to dbus",
                        );
                        proxy.read_cpu_stats().await?
                    }
                },
                None => proxy.read_cpu_stats().await?,
            };
        }
        if self.accounting.memory {
            stats.mem_stats = match self.cgroup.as_ref() {
                Some(cg) => match cg.read_memory_stats().await {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cgroup = %cg.to_string(),
                            name = %self.name,
                            "cgroup memory stats read failed; falling back to dbus",
                        );
                        proxy.read_memory_stats().await?
                    }
                },
                None => proxy.read_memory_stats().await?,
            };
        }

        Ok(stats)
    }
}
