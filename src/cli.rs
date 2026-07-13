use clap::{Arg, ArgAction, Command, crate_authors, crate_description, crate_version};
use prometheus_client::registry::Registry;
use std::{error::Error, process::exit};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use zbus_systemd::zbus::Connection;

use crate::metrics::UnitMetrics;
use crate::service::{Config, coordinator::Coordinator};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct AppConfig {
    listener_address: tokio_listener::ListenerAddress,
}

fn parse_config<'a, T: serde::Deserialize<'a>>(prefix: &str) -> Result<T, Box<dyn Error>> {
    let cfg_source = config::Config::builder()
        .add_source(
            config::Environment::with_prefix(prefix)
                .convert_case(config::Case::Snake)
                .list_separator(":")
                .with_list_parse_key("include_filters")
                .with_list_parse_key("exclude_filters")
                .try_parsing(true),
        )
        .build()?;

    cfg_source.try_deserialize().map_err(|err| {
        tracing::error!("Error in the provided configuration: {}", err);
        exit(2);
    })
}

fn setup_logger() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match std::env::var("RUST_LOG_STYLE") {
        // <3 systemd support
        Ok(s) if s == "SYSTEMD" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .event_format(SystemdFormat)
            .init(),
        _ => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .init(),
    };
}

struct SystemdFormat;

impl<S, N> tracing_subscriber::fmt::FormatEvent<S, N> for SystemdFormat
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> tracing_subscriber::fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: tracing_subscriber::fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let meta = event.metadata();
        let severity = match *meta.level() {
            tracing::Level::ERROR => 3,
            tracing::Level::WARN => 4,
            tracing::Level::INFO => 6,
            tracing::Level::DEBUG | tracing::Level::TRACE => 7,
        };
        write!(writer, "<{}>{}: ", severity, meta.target())?;

        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                write!(writer, "{}", span.name())?;
                let ext = span.extensions();
                if let Some(fields) = ext.get::<tracing_subscriber::fmt::FormattedFields<N>>() {
                    if !fields.is_empty() {
                        write!(writer, "{{{}}}", fields)?;
                    }
                }
                write!(writer, ": ")?;
            }
        }

        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
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

    let mut registry = Registry::with_prefix("systemd");
    let recorder = UnitMetrics::default();
    recorder.clone().register_metrics(&mut registry);

    let (tx, rx) = mpsc::channel(32);
    let app = crate::http::get_router(tx, recorder, registry);

    let system_conn = Connection::system().await.unwrap();
    let coordinator = Coordinator::new(system_conn, config);

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
    let joiner = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap()
    });

    coordinator
        .run(rx)
        .await
        .map_err(|err| {
            tracing::error!("Failed to run coordinator: {}", err);
            exit(4);
        })
        .unwrap();
    joiner.abort();
}
