use crate::cgroup::CGroup;
use crate::stats::{
    ResourceStats, TaskStats, UnitStatus,
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
        Ok(())
    }

    pub async fn collect_task_stats(&self) -> Result<TaskStats> {
        match &self.task_proxy {
            Some(p) => p.read_task_stats().await,
            None => Ok(TaskStats::default()),
        }
    }

    pub async fn collect_resource_stats(&self) -> Result<ResourceStats> {
        if self.running()
            && let Some(proxy) = &self.resource_proxy
        {
            proxy
                .read_resource_stats(self.cgroup.as_ref(), self.name.clone())
                .await
        } else {
            Ok(ResourceStats::default())
        }
    }
}
