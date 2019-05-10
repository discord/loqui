use chrono;
use failure::Error;
use fern;
use log;
use std::net::SocketAddr;

const ADDRESS: &str = "127.0.0.1:8080";

pub fn make_socket_address() -> SocketAddr {
    ADDRESS.parse().expect("Failed to parse address.")
}

pub fn configure_logging() -> Result<(), Error> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}:{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.level(),
                record.target(),
                record.line().unwrap_or(0),
                message
            ));
        })
        .level(log::LevelFilter::Debug)
        .level_for("tokio_core", log::LevelFilter::Warn)
        .level_for("tokio_reactor", log::LevelFilter::Warn)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
