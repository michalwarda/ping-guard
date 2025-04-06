# Ping Guard

Ping Guard is a simple cross-platform watchdog utility written in Rust. It launches a specified child process and monitors a UDP port for incoming signals (pings). If no signal is received within a configurable timeout period (currently hardcoded at 5 seconds internally), Ping Guard will terminate the child process and then exit itself.

## Features

- Launches and monitors a child process.
- Listens for simple UDP packets as keep-alive signals.
- Terminates the child process if no signal is received within the timeout.
- Configurable child process path and arguments.
- Configurable UDP listening address and port.
- Cross-platform (Linux, macOS, Windows).

## Usage

After building the application (see Building section), you can run the executable directly from your terminal.

The basic command structure is:

```bash
./ping-guard [OPTIONS] <BINARY_PATH> [CHILD_ARGS...]
```

Or on Windows:

```powershell
.\ping-guard.exe [OPTIONS] <BINARY_PATH> [CHILD_ARGS...]
```

**Arguments:**

- `<BINARY_PATH>` (Required): The path to the executable file of the child process you want to launch and monitor.
- `[CHILD_ARGS...]` (Optional): Any arguments you want to pass to the child process. These must come _after_ the `BINARY_PATH`.

**Options:**

- `-l <IP:PORT>`, `--listen-addr <IP:PORT>`: Specifies the IP address and port for the watchdog's UDP server to listen on for signals.
  - Default: `0.0.0.0:12345` (listens on all available network interfaces on port 12345).
- `-h`, `--help`: Prints help information.
- `-V`, `--version`: Prints version information.

**Examples:**

- **Linux/macOS:** Run the `sleep` command for 1000 seconds, monitored by the watchdog listening on the default port. Send a UDP packet to `127.0.0.1:12345` at least every 5 seconds to keep it alive.

  ```bash
  ./target/release/ping-guard /usr/bin/sleep 1000
  ```

- **Linux/macOS:** Run a custom application `/path/to/my/app` with arguments `--config file.conf`, listening on `127.0.0.1:9999`.

  ```bash
  ./target/release/ping-guard -l 127.0.0.1:9999 /path/to/my/app --config file.conf
  ```

- **Windows:** Run the `timeout.exe` command for 1000 seconds, monitored by the watchdog listening on the default port.

  ```powershell
  .\target\release\ping-guard.exe C:\Windows\System32\timeout.exe 1000
  ```

- **Windows:** Run a custom application `C:\path\to\app.exe` with a flag `--verbose`, listening only on the local machine (`127.0.0.1`) port `54321`.
  ```powershell
  .\target\release\ping-guard.exe --listen-addr 127.0.0.1:54321 C:\path\to\app.exe --verbose
  ```

**Sending Signals:**

You can send a UDP signal using various tools. The content of the UDP packet doesn't matter; its arrival is what resets the timer.

- Using `netcat` (`nc`):
  ```bash
  # Send an empty UDP packet (some versions of nc need input)
  echo "ping" | nc -u -w1 127.0.0.1 12345
  ```
- Using Python:
  ```python
  import socket
  sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
  sock.sendto(b'ping', ('127.0.0.1', 12345))
  ```

## Building

You need to have the Rust toolchain (including Cargo) installed. You can get it from [rustup.rs](https://rustup.rs/).

1.  **Clone the repository (if you haven't already):**
    ```bash
    git clone <repository-url>
    cd ping-guard
    ```
2.  **Build the project:**
    - For a development build (unoptimized, faster compilation):
      ```bash
      cargo build
      ```
      The executable will be located at `target/debug/ping-guard`.
    - For a release build (optimized, recommended for distribution/use):
      ```bash
      cargo build --release
      ```
      The executable will be located at `target/release/ping-guard`.

## Development

- **Running:** Use `cargo run` to build and run the application directly during development. Pass arguments after `--`.

  ```bash
  # Example: Run 'sleep 60' and listen on the default port
  cargo run -- /usr/bin/sleep 60

  # Example: Listen on a different port and run 'my_app --test'
  cargo run -- -l 127.0.0.1:8888 /path/to/my_app --test
  ```

- **Dependencies:** The project uses `tokio` for asynchronous operations (process handling, networking, timers) and `clap` for command-line argument parsing. Cargo handles dependency management.
- **Formatting and Linting:** Use `cargo fmt` to format the code and `cargo clippy` to check for common mistakes and style issues.
