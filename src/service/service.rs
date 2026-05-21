use super::{error::Error, unit::Unit};
use crate::cgroup::CGroup;
use crate::stats::{StatsRequest, UnitData};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use zbus_systemd::zbus::proxy::CacheProperties;
use zbus_systemd::zvariant::ObjectPath;
use zbus_systemd::{
    systemd1::{
        ManagerProxy, MountProxy, ScopeProxy, ServiceProxy, SliceProxy, SocketProxy, UnitProxy,
    },
    zbus::Connection,
};

struct UnitParser<'u> {
    unit_builder: zbus_systemd::zbus::proxy::Builder<'u, UnitProxy<'u>>,
    service_builder: zbus_systemd::zbus::proxy::Builder<'u, ServiceProxy<'u>>,
    slice_builder: zbus_systemd::zbus::proxy::Builder<'u, SliceProxy<'u>>,
    mount_builder: zbus_systemd::zbus::proxy::Builder<'u, MountProxy<'u>>,
    socket_builder: zbus_systemd::zbus::proxy::Builder<'u, SocketProxy<'u>>,
    scope_builder: zbus_systemd::zbus::proxy::Builder<'u, ScopeProxy<'u>>,
}

impl<'u> UnitParser<'u> {
    fn new(conn: &'u Connection) -> Self {
        let cache = CacheProperties::No;
        Self {
            unit_builder: UnitProxy::builder(conn).cache_properties(cache),
            service_builder: ServiceProxy::builder(conn).cache_properties(cache),
            slice_builder: SliceProxy::builder(conn).cache_properties(cache),
            mount_builder: MountProxy::builder(conn).cache_properties(cache),
            socket_builder: SocketProxy::builder(conn).cache_properties(cache),
            scope_builder: ScopeProxy::builder(conn).cache_properties(cache),
        }
    }

    async fn parse(&self, name: String, obj_path: ObjectPath<'u>) -> Result<Unit<'u>, Error> {
        let unit_proxy = self
            .unit_builder
            .clone()
            .path(obj_path.clone())?
            .build()
            .await?;

        let mut unit = Unit::new(name.clone(), unit_proxy);

        if name.ends_with(".service") {
            let proxy = self.service_builder.clone().path(obj_path)?.build().await?;
            let cgroup_path = proxy.control_group().await?;
            let cgroup = CGroup::new(&cgroup_path);
            unit = unit
                .with_cgroup(cgroup)
                .with_task_proxy(proxy.clone().into())
                .with_resource_proxy(proxy.into());
        } else if name.ends_with(".slice") {
            let proxy = self.slice_builder.clone().path(obj_path)?.build().await?;
            let cgroup_path = proxy.control_group().await?;
            let cgroup = CGroup::new(&cgroup_path);
            unit = unit.with_cgroup(cgroup).with_resource_proxy(proxy.into());
        } else if name.ends_with(".mount") {
            let proxy = self.mount_builder.clone().path(obj_path)?.build().await?;
            let cgroup_path = proxy.control_group().await?;
            let cgroup = CGroup::new(&cgroup_path);
            unit = unit.with_cgroup(cgroup).with_resource_proxy(proxy.into());
        } else if name.ends_with(".socket") {
            let proxy = self.socket_builder.clone().path(obj_path)?.build().await?;
            let cgroup_path = proxy.control_group().await?;
            let cgroup = CGroup::new(&cgroup_path);
            unit = unit.with_cgroup(cgroup).with_resource_proxy(proxy.into());
        } else if name.ends_with(".scope") {
            let proxy = self.scope_builder.clone().path(obj_path)?.build().await?;
            let cgroup_path = proxy.control_group().await?;
            let cgroup = CGroup::new(&cgroup_path);
            unit = unit.with_cgroup(cgroup).with_resource_proxy(proxy.into());
        }

        Ok(unit)
    }
}

pub struct SystemdExporter<'u> {
    parser: UnitParser<'u>,
    manager: ManagerProxy<'u>,
}

impl<'u> SystemdExporter<'u> {
    pub async fn new(conn: &'u Connection) -> Result<Self, Error> {
        let parser = UnitParser::new(conn);
        let manager = ManagerProxy::new(conn).await?;
        Ok(Self { parser, manager })
    }

    async fn load_all<'a>(&'u self) -> Result<HashMap<String, Unit<'a>>, Error>
    where
        'u: 'a,
    {
        let units = self.manager.list_units().await?;
        let mut all_units: HashMap<String, Unit<'a>> = Default::default();
        for (
            id,
            _desc,
            _load_state,
            _active_state,
            _sub_state,
            _following,
            obj_path,
            _job_id,
            _job_type,
            _job_object,
        ) in units
        {
            let unit: Unit<'a> = self.parser.parse(id.clone(), obj_path.into()).await?;
            all_units.insert(id, unit);
        }
        Ok(all_units)
    }

    // This function is only responsible for building the unit objects and
    // monitoring for state changes. It is not responsible for reading the unit stats.
    pub async fn monitor_units<'a>(
        &'u mut self,
        mut receiver: mpsc::Receiver<StatsRequest>,
    ) -> Result<(), Error>
    where
        'u: 'a,
    {
        // Perform an initial loading of all units.
        let mut units = self.load_all().await?;

        let mut receive_new = self.manager.receive_unit_new().await?;
        let mut receive_removed = self.manager.receive_unit_removed().await?;
        let mut receive_job_new = self.manager.receive_job_new().await?;
        let mut receive_job_removed = self.manager.receive_job_removed().await?;
        let mut receive_changed = self.manager.receive_unit_files_changed().await?;
        self.manager.subscribe().await?;

        // Listen for dbus unit events
        // Add/remove/update units as appropriate.
        loop {
            tokio::select! {
                Some(req) = receiver.recv() => {
                    let mut results = Vec::with_capacity(units.len());
                    for unit in units.values() {
                        let resource_stats = unit.collect_resource_stats().await.unwrap_or_default();
                        let task_stats = unit.collect_task_stats().await.unwrap_or_default();
                        results.push(UnitData {
                            name: unit.name.clone(),
                            machine: unit.machine.clone(),
                            status: unit.status.clone(),
                            resource_stats,
                            task_stats,
                        });
                    }
                    let _ = req.response.send(results);
                }
                Some(event) = receive_new.next() => {
                    let args = event.args()?;
                    let mut unit = self.parser.parse(args.id.clone(), args.unit.into()).await?;
                    unit.update_unit_status().await?;
                    units.insert(args.id, unit);
                }
                Some(event) = receive_removed.next() => {
                    let id = event.args()?.id;
                    units.remove(&id);
                }
                Some(event) = receive_job_new.next() => {
                    let id = event.args()?.unit;
                    match units.get_mut(&id) {
                        Some(unit) => {
                            unit.update_unit_status().await?;
                        },
                        None => {
                            // TODO log job event for unmonitored unit
                        },
                    }
                }
                Some(event) = receive_job_removed.next() => {
                    let id = event.args()?.unit;
                    match units.get_mut(&id) {
                        Some(unit) => {
                            unit.update_unit_status().await?;
                        },
                        None => {
                            // TODO log job event for unmonitored unit
                        },
                    }
                }
                Some(_) = receive_changed.next() => {
                    // No indication of what changed - we have to re-read all units
                    // TODO check if this overlaps with receive_new/receive_removed
                    let all_units = self.load_all().await?;
                    units = all_units;
                }
                else => { break }
            };
        }
        Ok(())
    }
}
