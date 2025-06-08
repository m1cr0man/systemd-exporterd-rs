pub mod http;
pub mod service;

#[cfg(feature = "cli")]
mod cli;

#[tokio::main]
async fn main() {
    #[cfg(not(feature = "cli"))]
    panic!("cli feature is not enabled");
    #[cfg(feature = "cli")]
    cli::main().await
}
