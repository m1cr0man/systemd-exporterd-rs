use super::{error::Error, unit::Unit};
use zbus_systemd::{
    systemd1::{ManagerProxy, ServiceProxy, UnitProxy},
    zbus::Connection,
};
pub struct SystemdExporter {}

impl SystemdExporter {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn load_units<'s, 'u>(&'s self) -> Result<Vec<Unit<'u>>, Error> {
        let conn = Connection::system().await?;
        let proxy = ManagerProxy::new(&conn).await?;
        let sproxy_builder = ServiceProxy::builder(&conn)
            .cache_properties(zbus_systemd::zbus::proxy::CacheProperties::No);
        let uproxy_builder = UnitProxy::builder(&conn)
            .cache_properties(zbus_systemd::zbus::proxy::CacheProperties::No);

        let units = proxy.list_units().await?;
        let mut parsed_units = Vec::with_capacity(units.len());
        for (
            name,
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
            let unit_proxy = uproxy_builder
                .clone()
                .path(obj_path.clone())?
                .build()
                .await?;

            // let path_str = obj_path.to_string();
            // println!("{}", path_str);

            let service_proxy = match name.ends_with(".service") {
                true => Some(sproxy_builder.clone().path(obj_path)?.build().await?),
                false => None,
            };

            let uwrapper = Unit::build(name, unit_proxy, service_proxy).await?;

            parsed_units.push(uwrapper);
        }

        Ok(parsed_units)
    }
}

impl From<super::Config> for SystemdExporter {
    fn from(_value: super::Config) -> Self {
        Self {}
    }
}
