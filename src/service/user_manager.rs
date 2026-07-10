use tokio::sync::Mutex;
use zbus_systemd::{
    login1::{ManagerProxy, UserProxy},
    zbus::{self, AuthMechanism, Connection, connection::Builder},
};

use super::error::Error;

pub struct UserInfo {
    pub uid: u32,
    pub name: String,
    pub runtime_path: String,
}

/// Lists active users from logind, skipping those whose manager is not reachable.
pub async fn list_active_users(system_conn: &Connection) -> Result<Vec<UserInfo>, Error> {
    let manager = ManagerProxy::new(system_conn).await?;
    let users = manager.list_users().await?;
    let mut result = Vec::new();
    for (uid, name, path) in users {
        let user_proxy = UserProxy::builder(system_conn).path(path)?.build().await?;
        let state = user_proxy.state().await?;
        if matches!(state.as_str(), "offline" | "closing") {
            continue;
        }
        let runtime_path = user_proxy.runtime_path().await?;
        if runtime_path.is_empty() {
            continue;
        }
        result.push(UserInfo {
            uid,
            name,
            runtime_path,
        });
    }
    Ok(result)
}

// Serializes process-wide seteuid transitions so parallel connect attempts
// don't clobber each other's euid.
static SETEUID_MUTEX: Mutex<()> = Mutex::const_new(());

fn is_running_as_root() -> bool {
    // SAFETY: geteuid is always safe to call.
    unsafe { libc::geteuid() == 0 }
}

fn should_try_seteuid(err: &Error) -> bool {
    match err {
        Error::Systemd { source } => matches!(
            source,
            zbus::Error::InputOutput(io)
                if matches!(
                    io.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::PermissionDenied
                )
        ),
    }
}

async fn build_connection(addr: &str) -> Result<Connection, Error> {
    let conn = Builder::address(addr)?
        .auth_mechanism(AuthMechanism::External)
        .build()
        .await?;
    Ok(conn)
}

/// Opens a connection to a per-user systemd manager via the session bus.
///
/// When run as root, most session dbus-daemons reject the EXTERNAL auth
/// handshake (see `docs/root-to-user-connection.md`). If we detect that,
/// fall back to a seteuid(uid) around the connect so the daemon sees the
/// target user's UID via SO_PEERCRED.
pub async fn connect_user_manager(info: &UserInfo) -> Result<Connection, Error> {
    let addr = format!("unix:path={}/bus", info.runtime_path);
    match build_connection(&addr).await {
        Ok(conn) => Ok(conn),
        Err(err) if should_try_seteuid(&err) && is_running_as_root() => {
            tracing::warn!(
                uid = info.uid,
                error = %err,
                "Session bus refused root; retrying with seteuid fallback"
            );
            connect_as_uid(&addr, info.uid).await
        }
        Err(err) => Err(err),
    }
}

async fn connect_as_uid(addr: &str, uid: u32) -> Result<Connection, Error> {
    let _guard = SETEUID_MUTEX.lock().await;

    // SAFETY: seteuid changes the process-wide effective UID. The static mutex
    // serializes callers; other tasks running during the await window below
    // will observe the switched euid, which is acceptable for our workload
    // (established connections don't re-check credentials, and cgroup reads
    // work under either UID).
    if unsafe { libc::seteuid(uid) } != 0 {
        let err = std::io::Error::last_os_error();
        return Err(zbus::Error::InputOutput(std::sync::Arc::new(err)).into());
    }

    let result = build_connection(addr).await;

    // Restore root. A failure here leaves the process in an inconsistent
    // state, so it's better to crash than continue with reduced privileges.
    if unsafe { libc::seteuid(0) } != 0 {
        let err = std::io::Error::last_os_error();
        panic!(
            "Failed to restore euid to 0 after seteuid fallback: {}",
            err
        );
    }

    result
}
