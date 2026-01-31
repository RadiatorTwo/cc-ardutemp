mod serial;
mod service;
mod state;

use crate::device_service::v1::device_service_server::DeviceServiceServer;
use crate::serial::SerialReader;
use crate::service::ArduTempService;
use crate::state::TemperatureState;
use anyhow::Result;
use clap::Parser;
use log::{LevelFilter, error, info};
use std::str::FromStr;
use systemd_journal_logger::{JournalLog, connected_to_journal};
use tokio::net::UnixListener;
use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;
use tonic::codegen::tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

pub const SERVICE_ID: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const ENV_CC_LOG: &str = "CC_LOG";
const DEFAULT_DEVICE: &str = "/dev/ttyACM0";
const DEFAULT_BAUD_RATE: u32 = 57600;

pub mod models {
    pub mod v1 {
        tonic::include_proto!("coolercontrol.models.v1");
    }
}
pub mod device_service {
    pub mod v1 {
        tonic::include_proto!("coolercontrol.device_service.v1");
    }
}

/// CoolerControl Device Service Plugin for Arduino Temperature Sensors
#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Enable debug logging
    #[clap(short, long)]
    debug: bool,

    /// Serial port device path
    #[clap(long, env = "ARDU_DEVICE", default_value = DEFAULT_DEVICE)]
    device: String,

    /// Serial port baud rate
    #[clap(long, env = "ARDU_BAUD", default_value_t = DEFAULT_BAUD_RATE)]
    baud: u32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Args = Args::parse();
    let run_token = setup_termination_signals();
    setup_logging(&args)?;

    info!("Starting {SERVICE_ID} v{VERSION}");
    info!("Device: {}, Baud: {}", args.device, args.baud);

    // Shared temperature state
    let state = TemperatureState::new();

    // Start serial reader thread
    let reader = SerialReader::new(args.device, args.baud, state.clone());
    let mut reader_handle = reader.spawn();

    // Create gRPC service
    let service = ArduTempService::new(state);

    // Setup Unix socket
    let uds_path = format!("/tmp/{SERVICE_ID}.sock");
    cleanup_uds(&uds_path).await;
    let uds = match UnixListener::bind(&uds_path) {
        Ok(listener) => listener,
        Err(err) => {
            error!(
                "Failed to bind to socket: {uds_path}. Make sure the service is running as root."
            );
            reader_handle.stop();
            return Err(err.into());
        }
    };

    info!("Listening on {}", uds_path);
    let uds_stream = UnixListenerStream::new(uds);
    Server::builder()
        .add_service(DeviceServiceServer::new(service))
        .serve_with_incoming_shutdown(uds_stream, run_token.cancelled())
        .await?;

    // Cleanup
    reader_handle.stop();
    cleanup_uds(&uds_path).await;
    info!("Shutdown complete");

    Ok(())
}

fn setup_logging(args: &Args) -> Result<()> {
    let log_level = if args.debug {
        LevelFilter::Debug
    } else if let Ok(log_lvl) = std::env::var(ENV_CC_LOG) {
        LevelFilter::from_str(&log_lvl).unwrap_or(LevelFilter::Info)
    } else {
        LevelFilter::Info
    };
    if connected_to_journal() {
        JournalLog::new()?
            .with_extra_fields(vec![("VERSION", VERSION)])
            .install()?;
        log::set_max_level(log_level);
    } else {
        env_logger::Builder::new().filter_level(log_level).init();
    }
    Ok(())
}

fn setup_termination_signals() -> CancellationToken {
    let run_token = CancellationToken::new();
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    let sigterm = async {
        signal::unix::signal(SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    let sigint = async {
        signal::unix::signal(SignalKind::interrupt())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    let sigquit = async {
        signal::unix::signal(SignalKind::quit())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    let sig_run_token = run_token.clone();
    tokio::task::spawn(async move {
        tokio::select! {
            () = ctrl_c => {},
            () = sigterm => {},
            () = sigint => {},
            () = sigquit => {},
        }
        sig_run_token.cancel();
        info!("Shutting down");
    });
    run_token
}

async fn cleanup_uds(uds_path: &str) {
    let _ = tokio::fs::remove_file(uds_path).await;
}
