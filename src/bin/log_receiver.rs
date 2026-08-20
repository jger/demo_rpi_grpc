use std::time::Duration;
use clap::Parser;
use colored::Colorize;
use tokio::time::sleep;

use minimum_hw_project::format_timestamp;
use minimum_hw_project::proto::telemetry_service_client::TelemetryServiceClient;
use minimum_hw_project::proto::{
    GetStatusRequest, LogEntry, LogLevel, StreamEventsRequest, StreamLogsRequest, SwitchEvent,
    SwitchId, SwitchState,
};

#[derive(Parser, Debug)]
#[command(name = "log_receiver")]
#[command(about = "Receives and formats gRPC telemetry and debug logs from Raspberry Pi 3")]
struct Args {
    /// Server gRPC URL endpoint
    #[arg(short, long, default_value = "http://127.0.0.1:50051")]
    server_url: String,

    /// Minimum log level to receive (debug, info, warn, error)
    #[arg(short, long, default_value = "debug")]
    min_level: String,

    /// Unique client identifier
    #[arg(short, long, default_value = "debug-receiver-cli")]
    client_id: String,
}

fn parse_log_level(level_str: &str) -> LogLevel {
    match level_str.to_lowercase().as_str() {
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warn" | "warning" => LogLevel::Warn,
        "error" => LogLevel::Error,
        _ => LogLevel::Debug,
    }
}

fn display_log_entry(entry: LogEntry) {
    let time_str = format_timestamp(entry.timestamp_unix_millis).dimmed();
    let level_enum = LogLevel::try_from(entry.level).unwrap_or(LogLevel::Debug);

    let level_tag = match level_enum {
        LogLevel::Debug => "[DEBUG]".cyan(),
        LogLevel::Info => "[INFO ]".bright_green(),
        LogLevel::Warn => "[WARN ]".bright_yellow().bold(),
        LogLevel::Error => "[ERROR]".bright_red().bold(),
    };

    let comp_tag = format!("[{}]", entry.component).magenta();
    println!("{} {} {:<12} {}", time_str, level_tag, comp_tag, entry.message);
}

fn display_switch_event(event: SwitchEvent) {
    let time_str = format_timestamp(event.timestamp_unix_millis).dimmed();
    let sw_id = SwitchId::try_from(event.switch_id).unwrap_or(SwitchId::Unspecified);
    let state = SwitchState::try_from(event.state).unwrap_or(SwitchState::Unspecified);

    let (sw_name, sw_color_fn): (&str, fn(&str) -> colored::ColoredString) = match sw_id {
        SwitchId::Switch1 => ("SW1 (GPIO 23)", |s| s.bright_magenta().bold()),
        SwitchId::Switch2 => ("SW2 (GPIO 24)", |s| s.bright_blue().bold()),
        _ => ("UNKNOWN SW", |s| s.dimmed()),
    };

    let state_badge = match state {
        SwitchState::Pressed => " ▼ PRESSED ".black().on_bright_green().bold(),
        SwitchState::Released => " ▲ RELEASED".black().on_bright_yellow().bold(),
        _ => " ? UNKNOWN ".black().on_white(),
    };

    let duration_note = if event.duration_pressed_millis > 0 {
        format!(" (held for {} ms)", event.duration_pressed_millis).italic().bright_cyan()
    } else {
        "".italic()
    };

    println!(
        "{} {} {:<16} {:<12} [Seq: #{:<3}]{}",
        time_str,
        "[EVENT]".black().on_bright_cyan().bold(),
        sw_color_fn(sw_name),
        state_badge,
        event.sequence_number,
        duration_note
    );
}

async fn run_client(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "Connecting to gRPC server...".dimmed());
    let mut client = TelemetryServiceClient::connect(args.server_url.clone()).await?;
    println!(
        "{} Connected to {}\n",
        "✓".bright_green().bold(),
        args.server_url.bright_cyan()
    );

    // Initial status query
    if let Ok(response) = client.get_status(GetStatusRequest {}).await {
        let status = response.into_inner();
        let s1 = SwitchState::try_from(status.switch_1_state).unwrap_or(SwitchState::Unspecified);
        let s2 = SwitchState::try_from(status.switch_2_state).unwrap_or(SwitchState::Unspecified);
        println!("{}", "── Initial Status ──────────────────────────────────────────".dimmed());
        println!(
            "  Uptime: {}s | SW1: {:?} (Total: {}) | SW2: {:?} (Total: {})",
            status.uptime_seconds.to_string().yellow(),
            s1,
            status.total_switch_1_presses.to_string().cyan(),
            s2,
            status.total_switch_2_presses.to_string().cyan()
        );
        println!("{}\n", "────────────────────────────────────────────────────────────".dimmed());
    }

    // Subscribe to Event Stream
    let mut event_client = client.clone();
    let client_id_events = args.client_id.clone();
    let events_handle = tokio::spawn(async move {
        let req = StreamEventsRequest {
            client_id: client_id_events,
        };
        match event_client.stream_events(req).await {
            Ok(res) => {
                let mut stream = res.into_inner();
                while let Ok(Some(event)) = stream.message().await {
                    display_switch_event(event);
                }
            }
            Err(e) => {
                eprintln!("{} Event stream error: {}", "[ERROR]".red(), e);
            }
        }
    });

    // Subscribe to Log Stream
    let mut log_client = client.clone();
    let client_id_logs = args.client_id.clone();
    let min_level = parse_log_level(&args.min_level);
    let logs_handle = tokio::spawn(async move {
        let req = StreamLogsRequest {
            client_id: client_id_logs,
            min_level: min_level.into(),
        };
        match log_client.stream_logs(req).await {
            Ok(res) => {
                let mut stream = res.into_inner();
                while let Ok(Some(log_entry)) = stream.message().await {
                    display_log_entry(log_entry);
                }
            }
            Err(e) => {
                eprintln!("{} Log stream error: {}", "[ERROR]".red(), e);
            }
        }
    });

    // Wait for streams or cancellation
    tokio::select! {
        _ = events_handle => {},
        _ = logs_handle => {},
        _ = tokio::signal::ctrl_c() => {
            println!("\n{}", "Exiting log receiver...".dimmed());
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("{}", "=========================================================".bright_cyan());
    println!("{}", "   Raspberry Pi 3 — Telemetry & Debug Log Receiver       ".bright_cyan().bold());
    println!("   Server: {} | Min Level: {}", args.server_url.yellow(), args.min_level.yellow());
    println!("{}", "=========================================================\n".bright_cyan());

    loop {
        if let Err(e) = run_client(&args).await {
            eprintln!(
                "{} Connection failed: {}. Retrying in 3s...",
                "[WARN]".yellow(),
                e
            );
            sleep(Duration::from_secs(3)).await;
        } else {
            break;
        }
    }
}
