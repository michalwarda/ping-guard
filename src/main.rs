use clap::Parser;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::time::{sleep, Instant};

/// A watchdog that launches a child process and terminates it
/// if no UDP signal is received on a specified port within a timeout.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The network address (IP:PORT) to listen on for UDP signals.
    #[arg(short, long, value_name = "IP:PORT", default_value = "0.0.0.0:12345")]
    listen_addr: String,

    /// The path to the child binary to execute.
    #[arg(value_name = "BINARY_PATH")]
    child_binary_path: PathBuf,

    /// Optional arguments to pass to the child binary.
    #[arg(last = true, value_name = "CHILD_ARGS")]
    child_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!(
        "Launching child process: {} with args: {:?}",
        cli.child_binary_path.display(),
        cli.child_args
    );
    println!("Listening for UDP signals on: {}", cli.listen_addr);

    // Launch the child process
    let mut command = Command::new(&cli.child_binary_path);
    command
        .args(&cli.child_args)
        .stdout(Stdio::piped()) // Pipe stdout/stderr if you want to see its output
        .stderr(Stdio::piped());

    let child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "Failed to spawn child process '{}': {}",
                cli.child_binary_path.display(),
                e
            );
            // Exit directly if spawning fails
            std::process::exit(1);
        }
    };

    println!("Child process launched (PID: {}).", child.id().unwrap_or(0));

    // Channel to notify the monitor about received signals
    let (signal_tx, signal_rx) = watch::channel(Instant::now());

    // --- Task 1: Listen for signals via UDP ---
    let listener_addr_clone = cli.listen_addr.clone();
    let signal_listener = tokio::spawn(async move {
        println!("Starting UDP signal listener on {}", listener_addr_clone);
        let socket = match UdpSocket::bind(&listener_addr_clone).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind UDP socket: {}", e);
                return;
            }
        };
        println!("UDP listener bound successfully.");

        let mut buf = [0; 10]; // Small buffer

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((_len, src_addr)) => {
                    let now = Instant::now();
                    println!("UDP Signal received from: {} at: {:?}", src_addr, now);
                    if signal_tx.send(now).is_err() {
                        eprintln!("Monitor task receiver dropped, stopping UDP listener.");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving UDP packet: {}", e);
                    break;
                }
            }
        }
    });

    // --- Task 2: Monitor for timeout ---
    let monitor_task = tokio::spawn(monitor_timeout(child, signal_rx));

    let monitor_result = monitor_task.await?;
    signal_listener.abort(); // Stop the listener task if monitor finishes

    println!("Monitor task finished: {:?}", monitor_result);

    Ok(())
}

async fn monitor_timeout(
    mut child: Child,
    mut signal_rx: watch::Receiver<Instant>,
) -> Result<(), String> {
    let timeout_duration = Duration::from_secs(5);
    println!(
        "Monitoring for signal timeout ({} seconds)...",
        timeout_duration.as_secs()
    );

    loop {
        let last_signal_time = *signal_rx.borrow();
        let elapsed = Instant::now().duration_since(last_signal_time);

        if elapsed >= timeout_duration {
            eprintln!(
                "Timeout detected! No signal received for {:.2?} (limit: {:.2?}).",
                elapsed, timeout_duration
            );

            println!(
                "Terminating child process (PID: {})...",
                child.id().unwrap_or(0)
            );
            if let Err(e) = child.kill().await {
                eprintln!("Failed to kill child process: {}", e);
            } else {
                println!("Child process terminated.");
            }

            println!("Exiting watchdog.");
            std::process::exit(1);
        }

        let time_to_next_check = timeout_duration.saturating_sub(elapsed);

        tokio::select! {
            changed_result = signal_rx.changed() => {
                if changed_result.is_err() {
                    eprintln!("Signal sender dropped. Stopping monitor.");
                    let _ = child.kill().await;
                    return Err("Signal listener stopped unexpectedly".to_string());
                }
                 println!("Monitor notified of new signal.");
            }
            _ = sleep(time_to_next_check) => {
                 println!("Potential timeout check after sleep.");
            }
        }
    }
}
