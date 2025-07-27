use std::path::PathBuf;

use clap::Parser;
use evdev::{
    AttributeSet, BusType, Device, EventType, InputId, KeyCode, MiscCode, PropType,
    RelativeAxisCode, SynchronizationCode, UinputAbsSetup, uinput::VirtualDevice,
};
use figment::providers::Format;

use self::{config::*, logic::*, notify::SdNotify};

mod config;
mod logic;
mod notify;
mod state;
mod utils;

#[derive(clap::Parser)]
struct Cli {
    /// Path to the config file.
    #[arg(long, short, default_value = "tweakpoint.toml")]
    config: PathBuf,
    /// Dump the active config and exit.
    #[arg(long)]
    dump_config: bool,
    #[arg(long)]
    /// List known key codes and exit.
    list_keys: bool,
    /// List known relative axis codes and exit.
    #[arg(long)]
    list_relative_axes: bool,
    /// List known bus types and exit
    #[arg(long)]
    list_bus_types: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .pretty()
        .init();

    let cli = Cli::parse();

    if cli.list_keys {
        for i in 0..u16::MAX {
            let fmt = format!("{:?}", KeyCode(i));
            if !fmt.starts_with("unknown key:") {
                println!("{fmt}");
            }
        }
    }

    if cli.list_relative_axes {
        for i in 0..u16::MAX {
            let fmt = format!("{:?}", RelativeAxisCode(i));
            if !fmt.starts_with("unknown key:") {
                println!("{fmt}");
            }
        }
    }

    if cli.list_bus_types {
        for i in 0..u16::MAX {
            let fmt = format!("{:?}", BusType(i));
            if !fmt.starts_with("unknown key:") {
                println!("{fmt}");
            }
        }
    }

    if cli.list_relative_axes || cli.list_keys || cli.list_bus_types {
        return Ok(());
    }

    let config: Config = figment::Figment::new()
        .join(figment::providers::Toml::file(&cli.config))
        .extract()?;

    if cli.dump_config {
        println!("{}", toml::to_string(&config)?);
        return Ok(());
    }

    let mut device = Device::open(&config.device)?;
    device.grab()?;

    tracing::debug!(?device, "Opened and grabbed device");

    let mut dev = VirtualDevice::builder()?
        .name(&config.name)
        .input_id(InputId::new(
            config.bus,
            config.vendor_id,
            config.product_id,
            config.product_version,
        ))
        .with_properties(&AttributeSet::from_iter([PropType::POINTER]))?
        .with_keys(&AttributeSet::from_iter((0..560).map(KeyCode)))?
        .with_relative_axes(&AttributeSet::from_iter(
            (if config.hi_res_enabled {
                0..=12
            } else {
                0..=10
            })
            .map(RelativeAxisCode),
        ))?;
    for (axis, info) in device.get_absinfo()? {
        dev = dev.with_absolute_axis(&UinputAbsSetup::new(axis, info))?;
    }
    if let Some(ff) = device.supported_ff() {
        dev = dev.with_ff(ff)?;
    }
    if let Some(switch) = device.supported_switches() {
        dev = dev.with_switches(switch)?;
    }
    let dev = dev.build()?;

    tracing::debug!(?dev, "Created virtual device");

    let mut udev_stream = dev.into_event_stream()?;

    let socket = if let Some(path) = &config.socket_path {
        if let Ok(true) = tokio::fs::try_exists(path).await {
            tokio::fs::remove_file(path).await?;
        }
        Some(tokio::net::UnixListener::bind(path)?)
    } else {
        None
    };

    let mut controller = Controller::new(config);

    let state_vec_tx = if let Some(socket) = socket {
        let (state_vec_tx, state_vec_rx) = {
            let mut state = vec![];
            controller.state_vec(&mut state);
            tokio::sync::watch::channel(state)
        };
        tokio::spawn(handle_socket(socket, state_vec_rx));
        Some(state_vec_tx)
    } else {
        None
    };

    let mut stream = device.into_event_stream()?;
    let mut buf = vec![];

    tracing::debug!("Starting main loop");

    SdNotify::new()?.ready().await?;

    loop {
        let mut ev = tokio::select! {
            biased;
            _ = controller.next_events(&mut buf) => {
                tracing::trace!(?buf, "Controller emitted events");
                udev_stream.device_mut().emit(&buf)?;
                buf.clear();
                continue;
            }
            evt = udev_stream.next_event() => {
                tracing::trace!(?evt, "Event virtual -> physical");
                stream.device_mut().send_events(&[evt?])?;
                continue;
            }
            evt = stream.next_event() => {
                tracing::trace!(?evt, "Event physical -> virtual");
                evt?
            }
        };

        loop {
            match ev.event_type() {
                EventType::SYNCHRONIZATION if ev.code() == SynchronizationCode::SYN_REPORT.0 => {
                    break;
                }
                EventType::KEY => controller.button(KeyCode(ev.code()), ev.value()),
                EventType::RELATIVE => controller.relative(RelativeAxisCode(ev.code()), ev.value()),
                EventType::MISC if ev.code() == MiscCode::MSC_SCAN.0 => {
                    tracing::trace!(?ev, "Filtered out MSC_SCAN event");
                }
                _ => controller.passthrough(ev),
            }
            ev = stream.next_event().await?;
            tracing::trace!(?ev, "Event physical -> virtual");
        }
        controller.end_transaciton();
        if let Some(state_vec_tx) = &state_vec_tx {
            state_vec_tx.send_modify(|x| {
                x.clear();
                controller.state_vec(x);
            });
        }
    }
}

async fn handle_socket(
    socket: tokio::net::UnixListener,
    state_vec_rx: tokio::sync::watch::Receiver<Vec<u8>>,
) {
    use std::time::Instant;
    let mut limit = (Instant::now(), 0u8);
    loop {
        let mut conn = match socket.accept().await {
            Ok((conn, _)) => conn,
            Err(e) => {
                tracing::error!(error = %e, "Failed to accept socket connection");
                if limit.1 > 10 && limit.0.elapsed().as_secs_f32() < 5.0 {
                    tracing::error!(
                        "Socket connections failing too often; bailing, socket is disabled"
                    );
                    break;
                } else if limit.1 == 0 || limit.0.elapsed().as_secs_f32() >= 5.0 {
                    limit = (Instant::now(), 1);
                } else {
                    // limit <= 10 elapsed < 5.0
                    limit.1 = limit.1.saturating_add(1);
                }
                continue;
            }
        };
        let mut state_vec_rx = state_vec_rx.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            use tokio::io::AsyncWriteExt;
            let (mut rx, mut tx) = conn.split();
            let writer = async {
                loop {
                    let data = state_vec_rx.borrow_and_update().clone();
                    if tx.write_all(&data).await.is_err() {
                        break;
                    }
                    if state_vec_rx.changed().await.is_err() {
                        break;
                    };
                }
            };
            let reader = async {
                loop {
                    let mut buf = [0; 1024];
                    match rx.read(&mut buf).await {
                        Ok(0) => break, // client disconnect
                        Ok(_) => {}     // ignore
                        Err(e) => {
                            tracing::error!(error = %e, "Error reading from the socket");
                            break;
                        }
                    }
                }
            };
            tokio::select! {
                _ = reader => {}
                _ = writer => {}
            }
        });
    }
}
