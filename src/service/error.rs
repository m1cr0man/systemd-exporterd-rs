use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(transparent)]
    Systemd { source: zbus_systemd::zbus::Error },
}

// impl From<zbus_systemd::zbus::Error> for Error {
//     fn from(source: zbus_systemd::zbus::Error) -> Self {
//         SystemdSnafu{}
//     }
// }
