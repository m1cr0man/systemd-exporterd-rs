use axum::Router;
use clap::{Arg, ArgAction, Command, crate_authors, crate_description, crate_version};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use std::env;
use std::io::Write;
use std::time::Duration;
use std::{error::Error, process::exit, sync::Arc};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tokio::time::sleep;

use crate::metrics::UnitMetrics;
use crate::service::{Config, SystemdExporter};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct AppConfig {
    listener_address: tokio_listener::ListenerAddress,
}

fn parse_config<'a, T: serde::Deserialize<'a>>(prefix: &str) -> Result<T, Box<dyn Error>> {
    let cfg_source = config::Config::builder()
        .add_source(
            config::Environment::with_prefix(prefix)
                .convert_case(config::Case::ScreamingSnake)
                .try_parsing(true),
        )
        .build()?;

    cfg_source.try_deserialize().map_err(|err| {
        tracing::error!("Error in the provided configuration: {}", err);
        exit(2);
    })
}

fn app(recv: Arc<RwLock<String>>) -> Router {
    crate::http::get_router(recv)
}

fn setup_logger() {
    // Set a default level. TODO investigate again
    // if env::var("RUST_LOG").is_err() {
    //     env::set_var("RUST_LOG", "info")
    // }

    // Adapted from env_logger examples. <3 Systemd support
    match std::env::var("RUST_LOG_STYLE") {
        Ok(s) if s == "SYSTEMD" => env_logger::builder()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "<{}>{}: {}",
                    match record.level() {
                        log::Level::Error => 3,
                        log::Level::Warn => 4,
                        log::Level::Info => 6,
                        log::Level::Debug => 7,
                        log::Level::Trace => 7,
                    },
                    record.target(),
                    record.args()
                )
            })
            .init(),
        _ => pretty_env_logger::init(),
    };
}

async fn monitor(dest: Arc<RwLock<String>>, service: SystemdExporter) {
    let mut registry = Registry::default();
    let mut recorder = UnitMetrics::default();
    recorder.clone().register_metrics(&mut registry);
    let mut units = service.load_units().await.unwrap();
    loop {
        let mut new_units = Vec::with_capacity(units.len());
        let mut tset = JoinSet::from_iter(units.into_iter().map(|unit| unit.collect_stats()));
        recorder.new_batch();
        while let Some(res) = tset.join_next().await {
            let unit = res.unwrap().unwrap();
            recorder.record_unit(&unit);
            new_units.push(unit);
        }
        let mut buffer = String::new();
        if let Err(err) = encode(&mut buffer, &registry) {
            tracing::error!("Failed to encode registry: {}", err);
        }
        *(dest.write().await) = buffer;
        units = new_units;
        sleep(Duration::from_secs(5)).await;
    }
}

pub(crate) async fn main() {
    let cli = Command::new("SystemdExporter")
        .about(format!(
            "{}\n{} {}",
            crate_description!(),
            "Configuration is managed using environment variables.",
            "See the docs for more information.",
        ))
        .arg(
            Arg::new("check")
                .action(ArgAction::SetTrue)
                .short('c')
                .long("check")
                .help("Check the configuration"),
        )
        .version(crate_version!())
        .author(crate_authors!("\n"));

    let args = cli.get_matches();

    setup_logger();

    let app_config: AppConfig = parse_config("SDED").unwrap();
    let config: Config = parse_config("SDED").unwrap();
    let user_opts: tokio_listener::UserOptions = parse_config("SDED_LISTENER").unwrap();

    if args.get_flag("check") {
        tracing::info!("Configuration is valid.");
        exit(0);
    }

    let datalock = Arc::new(RwLock::default());
    let service = SystemdExporter::from(config.clone());
    let joiner = tokio::spawn(monitor(datalock.clone(), service));
    let app = app(datalock);

    // Start the web server
    let listener = tokio_listener::Listener::bind(
        &app_config.listener_address,
        &tokio_listener::SystemOptions::default(),
        &user_opts,
    )
    .await
    .map_err(|err| {
        tracing::error!("Failed to configure listener: {}", err);
        exit(3);
    })
    .unwrap();

    tracing::info!("Listening on {}", app_config.listener_address);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
