pub mod cgroup;
pub(crate) mod constants;
pub mod http;
pub mod metrics;
pub mod service;
pub mod stats;

#[cfg(feature = "cli")]
mod cli;

#[tokio::main]
async fn main() {
    #[cfg(not(feature = "cli"))]
    panic!("cli feature is not enabled");
    #[cfg(feature = "cli")]
    cli::main().await
}
