use clap::Parser;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::time::{sleep, Instant};

// Platform-specific imports
#[cfg(unix)]
use libc;
// Signal handling
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "IP:PORT", default_value = "0.0.0.0:12345")]
    listen_addr: String,

    #[arg(short, long, value_name = "SECONDS", default_value_t = 5)]
    timeout_secs: u64,

    #[arg(value_name = "BINARY_PATH")]
    child_binary_path: PathBuf,

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
    println!("Timeout set to: {} seconds", cli.timeout_secs);

    if cli.timeout_secs == 0 {
        eprintln!("Error: Timeout must be greater than 0 seconds.");
        std::process::exit(1);
    }
    let timeout_duration = Duration::from_secs(cli.timeout_secs);

    // --- Setup command with platform-specific process group handling ---
    let mut command = Command::new(&cli.child_binary_path);
    command
        .args(&cli.child_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        // Place the child process in its own process group.
        // The PGID will be the same as the child's PID.
        command.process_group(0);
    }

    // --- Spawn the child process ---
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!(
                "Failed to spawn child process '{}': {}",
                cli.child_binary_path.display(),
                e
            );
            std::process::exit(1);
        }
    };
    // Get the PID *before* potentially moving the child into the monitor task
    let child_pid = match child.id() {
        Some(pid) => pid,
        None => {
            eprintln!("Error: Could not get PID of spawned child process.");
            // Ensure kill is attempted if spawn succeeded but PID failed
            if let Err(kill_err) = child.start_kill() {
                eprintln!(
                    "Error attempting to kill child process after failing to get PID: {}",
                    kill_err
                );
            }
            // Don't await here indefinitely, just try to wait briefly
            let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
            std::process::exit(1);
        }
    };
    println!("Child process launched (PID: {}).", child_pid);

    // Channel to notify the monitor about received signals
    let (signal_tx, signal_rx) = watch::channel(Instant::now());

    // Create a channel for propagating termination signals
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // --- Task 0: Set up signal handling ---
    let child_pid_for_signal = child_pid;
    tokio::spawn(async move {
        handle_termination_signals(child_pid_for_signal, shutdown_tx).await;
    });

    // --- Task 1: Listen for signals via UDP ---
    let listener_addr_clone = cli.listen_addr.clone();
    let signal_listener = tokio::spawn(async move {
        println!("Starting UDP signal listener on {}", listener_addr_clone);
        let socket = match UdpSocket::bind(&listener_addr_clone).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind UDP socket: {}", e);
                return; // Exit this task if binding fails
            }
        };
        println!("UDP listener bound successfully.");
        let mut buf = [0; 10]; // Small buffer suffices
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((_len, _src_addr)) => {
                    let now = Instant::now();
                    // Optional: Reduce log noise by commenting this out in production
                    // println!("UDP Signal received from: {} at: {:?}", src_addr, now);
                    if signal_tx.send(now).is_err() {
                        // This happens if the monitor task has already exited
                        eprintln!("Monitor task receiver dropped, stopping UDP listener.");
                        break;
                    }
                }
                Err(e) => {
                    // Errors here might indicate network issues or socket closure
                    eprintln!("Error receiving UDP packet: {}. Stopping listener.", e);
                    break;
                }
            }
        }
    });

    // --- Task 2: Monitor for timeout and child exit ---
    let monitor_task = tokio::spawn(monitor_timeout(
        child,
        signal_rx,
        timeout_duration,
        child_pid,
        shutdown_rx,
    ));

    // Wait for the monitor task to complete (it will exit the process internally)
    // Or handle potential errors from the monitor task itself
    if let Err(e) = monitor_task.await {
        eprintln!("Monitor task failed: {:?}", e);
        // Abort the listener if it's still running
        signal_listener.abort();
        return Err(e.into()); // Propagate join error if any
    }

    // In the normal case, monitor_task calls std::process::exit, so this part might not be reached.
    println!("Watchdog main function finished cleanly (unexpected).");
    signal_listener.abort(); // Ensure listener stops if monitor task somehow returned Ok

    Ok(())
}

/// Handles termination signals and initiates child process cleanup
async fn handle_termination_signals(child_pid: u32, shutdown_tx: tokio::sync::oneshot::Sender<()>) {
    println!("Setting up signal handlers for graceful shutdown...");

    #[cfg(unix)]
    {
        // Set up handlers for common termination signals on Unix
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
        let mut sighup = signal(SignalKind::hangup()).expect("Failed to set up SIGHUP handler");

        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM signal. Initiating shutdown...");
            }
            _ = sigint.recv() => {
                println!("Received SIGINT signal (Ctrl+C). Initiating shutdown...");
            }
            _ = sighup.recv() => {
                println!("Received SIGHUP signal. Initiating shutdown...");
            }
        }
    }

    #[cfg(windows)]
    {
        // On Windows, we handle Ctrl+C and Ctrl+Break
        let mut ctrl_c = windows::ctrl_c().expect("Failed to set up Ctrl+C handler");
        let mut ctrl_break = windows::ctrl_break().expect("Failed to set up Ctrl+Break handler");

        tokio::select! {
            _ = ctrl_c.recv() => {
                println!("Received Ctrl+C signal. Initiating shutdown...");
            }
            _ = ctrl_break.recv() => {
                println!("Received Ctrl+Break signal. Initiating shutdown...");
            }
        }
    }

    // Send shutdown signal to monitor task
    if shutdown_tx.send(()).is_err() {
        // If the receiver is dropped, it means the monitor task has already exited.
        // In that case, we'll try to kill the child process directly.
        println!("Monitor task already exited. Attempting to kill child process directly.");

        #[cfg(unix)]
        unsafe {
            println!("Sending SIGKILL to process group {}.", child_pid as i32);
            // Safety: We're sending a signal to a valid process group
            libc::killpg(child_pid as i32, libc::SIGKILL);
        }

        #[cfg(windows)]
        {
            println!("Windows: Cannot directly kill the child process outside the original Child structure.");
            // On Windows, we don't have a direct way to kill a process by PID in this context.
            // A more comprehensive solution would require the windows_sys crate to use TerminateProcess.
        }
    } else {
        println!("Shutdown signal sent to monitor task. Waiting for cleanup to complete...");
        // Give the monitor a moment to handle the shutdown
        sleep(Duration::from_millis(200)).await;
    }

    // Exit the process
    println!("Signal handler exiting the watchdog process.");
    std::process::exit(130); // 128 + signal number (SIGINT=2)
}

/// Attempts to kill the process group on Unix, or just the process on Windows.
/// Takes ownership of the Child to ensure it's handled correctly.
async fn kill_child_process_tree(mut child: Child, pid: u32) {
    println!(
        "Terminating child process{} (PID: {})...",
        if cfg!(unix) { " group" } else { "" },
        pid
    );

    #[cfg(unix)]
    unsafe {
        // Send SIGKILL to the entire process group.
        // PGID is the same as PID because we used command.process_group(0).
        let pgid = pid as i32; // Cast PID to i32 for libc functions
        println!("Attempting to send SIGKILL to process group {}.", pgid);
        if libc::killpg(pgid, libc::SIGKILL) == -1 {
            // EINVAL: pgid <= 0. ESRCH: No process/group found. EPERM: No permission.
            let err = std::io::Error::last_os_error();
            eprintln!(
                "Failed to kill process group {} with killpg: {}. Falling back to killing PID {}.",
                pgid, err, pid
            );
            // Fallback: Attempt to kill the direct child process if killpg fails or if the process is not in the group somehow
            if let Err(e) = child.start_kill() {
                // `start_kill` is non-blocking
                eprintln!(
                    "Fallback attempt to kill child process {} failed: {}",
                    pid, e
                );
            } else {
                println!("Fallback kill signal sent to PID {}.", pid);
            }
        } else {
            println!("Sent SIGKILL to process group {}.", pgid);
        }
    }

    #[cfg(windows)]
    {
        // On Windows, child.kill() or start_kill() terminates the direct process.
        // Terminating grandchildren requires Job Objects, which is more complex.
        println!("Attempting to kill process {} (Windows).", pid);
        if let Err(e) = child.start_kill() {
            eprintln!("Failed to initiate kill for child process {}: {}", pid, e);
        } else {
            println!("Kill signal sent to PID {}.", pid);
        }
    }

    // Give a brief moment for the signal to take effect.
    sleep(Duration::from_millis(100)).await;

    // Optionally, explicitly wait for the child to exit after sending kill signal
    match child.try_wait() {
        Ok(Some(status)) => println!(
            "Child process confirmed exit after kill signal with status: {}",
            status
        ),
        Ok(None) => {
            println!(
                "Child process still running shortly after kill signal, continuing watchdog exit."
            );
            // It might take longer, but the watchdog is exiting anyway.
        }
        Err(e) => eprintln!("Error checking child process status after kill: {}", e),
    }
}

/// Monitors for signal timeout or child process exit. Exits the watchdog process.
async fn monitor_timeout(
    mut child: Child, // Takes ownership
    mut signal_rx: watch::Receiver<Instant>,
    timeout_duration: Duration,
    child_pid: u32,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    // Return type might not be reached due to std::process::exit
    println!(
        "Monitoring for signal timeout ({:.2?}) and child process ({}) exit...",
        timeout_duration, child_pid
    );

    loop {
        // Calculate time until next potential timeout *relative to the last known signal*
        let last_signal_time = *signal_rx.borrow();
        let elapsed_since_last_signal = Instant::now().duration_since(last_signal_time);
        // If timeout already passed, sleep for a very short duration just to yield
        let time_to_next_check = timeout_duration.saturating_sub(elapsed_since_last_signal);

        tokio::select! {
            // Biased select ensures we check child exit/signal first if ready
            biased;

            // NEW BRANCH: Check for shutdown signal from signal handlers
            _ = &mut shutdown_rx => {
                println!("Received shutdown signal. Terminating child process...");
                kill_child_process_tree(child, child_pid).await;
                println!("Exiting watchdog due to shutdown signal.");
                std::process::exit(0);
            }

            // Branch 1: Wait for the child process to exit on its own
            // Note: child.wait() consumes the `child` variable when polled the first time.
            wait_result = child.wait() => {
                 match wait_result {
                    Ok(status) => {
                        println!("Child process exited on its own with status: {}. Exiting watchdog.", status);
                        std::process::exit(0); // Exit normally
                    }
                    Err(e) => {
                        eprintln!("Error waiting for child process exit: {}. Exiting watchdog.", e);
                        // Child might be unrecoverable, exit watchdog with error code
                        std::process::exit(2); // Exit with different code for error
                    }
                 }
                 // If wait() completed, the child variable is consumed, so we must exit.
                 // The std::process::exit calls above handle this.
            }

            // Branch 2: Wait for a new signal notification
            changed_result = signal_rx.changed() => {
                if changed_result.is_err() {
                    // The sender (signal listener) was dropped. This is unexpected.
                    eprintln!("Signal sender dropped unexpectedly. Terminating child and exiting watchdog.");
                    // Attempt to kill the child process tree just in case.
                    // Since wait() hasn't completed, `child` should still be available here.
                    kill_child_process_tree(child, child_pid).await; // kill_child_process_tree consumes child
                    std::process::exit(3); // Exit with code indicating listener failure
                }
                // New signal received, print status and loop continues.
                 let _latest_signal_time = *signal_rx.borrow(); // Get the updated time
                 // Optional: Reduce log noise
                 // println!("Monitor notified of new signal received at {:?}.", latest_signal_time);
                 // No action needed here, the loop will recalculate sleep duration
            }

             // Branch 3: Check for timeout ONLY if the sleep duration completes
            _ = sleep(time_to_next_check) => {
                // Re-verify timeout condition *after* sleep completes, using the latest signal time again.
                // This guards against race conditions where a signal arrived *during* the sleep.
                let current_elapsed = Instant::now().duration_since(*signal_rx.borrow());
                if current_elapsed >= timeout_duration {
                     eprintln!(
                        "Timeout detected! No signal received for ~{:.2?} (limit: {:.2?}). Terminating child.",
                        current_elapsed, // Display actual elapsed time
                        timeout_duration
                    );
                    // Terminate the child process tree
                    // Since wait() hasn't completed, `child` should still be available here.
                    kill_child_process_tree(child, child_pid).await; // kill_child_process_tree consumes child

                    println!("Exiting watchdog due to timeout.");
                    std::process::exit(1); // Exit with non-zero for timeout
                } else {
                    // If we woke up from sleep but the condition is no longer met,
                    // it means a signal arrived very recently. Log this and continue.
                    println!("Potential timeout check passed (signal received during sleep).");
                }
            }

        }
    }
}
