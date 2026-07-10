use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use zbus_systemd::{
    login1::{ManagerProxy as Login1ManagerProxy, UserProxy},
    zbus::Connection,
};

use super::error::Error;
use super::user_manager::{UserInfo, connect_user_manager, list_active_users};
use super::{Config, monitor_manager};
use crate::stats::StatsRequest;

#[derive(Hash, PartialEq, Eq)]
enum Scope {
    System,
    User(u32),
}

pub struct Coordinator {
    system_conn: Connection,
    config: Config,
    senders: HashMap<Scope, mpsc::Sender<StatsRequest>>,
}

impl Coordinator {
    pub fn new(system_conn: Connection, config: Config) -> Self {
        Self {
            system_conn,
            config,
            senders: HashMap::new(),
        }
    }

    fn spawn_manager(&mut self, conn: Connection, scope_key: Scope, scope_label: String) {
        let (tx, rx) = mpsc::channel(8);
        let config = self.config.clone();
        tokio::spawn(async move {
            if let Err(err) = monitor_manager(conn, scope_label.clone(), config, rx).await {
                tracing::error!(scope = %scope_label, "Manager task exited with error: {}", err);
            }
        });
        self.senders.insert(scope_key, tx);
    }

    async fn spawn_user(&mut self, info: UserInfo) {
        let scope_label = format!("user@{}", info.uid);
        tracing::debug!(uid = info.uid, "Connecting to user");
        match connect_user_manager(&info).await {
            Ok(conn) => {
                tracing::info!(uid = info.uid, "Spawning user manager task");
                self.spawn_manager(conn, Scope::User(info.uid), scope_label);
            }
            Err(err) => {
                tracing::warn!(uid = info.uid, "Failed to connect to user manager: {}", err);
            }
        }
    }

    async fn handle_scrape_request(&self, req: StatsRequest) {
        let mut all_data = Vec::new();
        for sender in self.senders.values() {
            let (tx, rx) = oneshot::channel();
            if sender.send(StatsRequest { response: tx }).await.is_ok()
                && let Ok(data) = rx.await
            {
                all_data.extend(data);
            }
        }
        let _ = req.response.send(all_data);
    }

    pub async fn run(mut self, mut receiver: mpsc::Receiver<StatsRequest>) -> Result<(), Error> {
        let system_conn = Connection::system().await?;
        self.spawn_manager(system_conn, Scope::System, "system".to_string());

        let enable_users = self.config.enable_user_managers.unwrap_or(false);

        if enable_users {
            let users = list_active_users(&self.system_conn).await?;
            for user in users {
                self.spawn_user(user).await;
            }

            // Clone the connection so login1 proxy borrows a local, not self.system_conn,
            // allowing self to be mutably borrowed inside the select! arms.
            let login1_conn = self.system_conn.clone();
            let login1 = Login1ManagerProxy::new(&login1_conn).await?;
            let mut user_new = login1.receive_user_new().await?;
            let mut user_removed = login1.receive_user_removed().await?;

            loop {
                tokio::select! {
                    Some(req) = receiver.recv() => {
                        self.handle_scrape_request(req).await;
                    }
                    Some(event) = user_new.next() => {
                        let args = event.args()?;
                        tracing::info!(uid = args.uid, "New user session detected");
                        let user_proxy = match UserProxy::builder(&login1_conn)
                            .path(args.object_path.clone())?
                            .build()
                            .await
                        {
                            Ok(p) => p,
                            Err(err) => {
                                tracing::warn!(uid = args.uid, "Failed to build UserProxy: {}", err);
                                continue;
                            }
                        };
                        let state = user_proxy.state().await.unwrap_or_default();
                        if matches!(state.as_str(), "offline" | "closing") {
                            continue;
                        }
                        let runtime_path = match user_proxy.runtime_path().await {
                            Ok(p) => p,
                            Err(err) => {
                                tracing::warn!(uid = args.uid, "Failed to read runtime_path: {}", err);
                                continue;
                            }
                        };
                        if runtime_path.is_empty() {
                            continue;
                        }
                        let info = UserInfo {
                            uid: args.uid,
                            name: String::new(),
                            runtime_path,
                        };
                        self.spawn_user(info).await;
                    }
                    Some(event) = user_removed.next() => {
                        let args = event.args()?;
                        tracing::info!(uid = args.uid, "User session removed");
                        self.senders.remove(&Scope::User(args.uid));
                    }
                    else => break,
                }
            }
        } else {
            loop {
                tokio::select! {
                    Some(req) = receiver.recv() => {
                        self.handle_scrape_request(req).await;
                    }
                    else => break,
                }
            }
        }

        Ok(())
    }
}
