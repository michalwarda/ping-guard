# Ping Guard

Ping Guard is a simple cross-platform watchdog utility written in Rust. It launches a specified child process and monitors a UDP port for incoming signals (pings). If no signal is received within a configurable timeout period, Ping Guard will terminate the child process and then exit itself.

## Features

- Launches and monitors a child process.
- Listens for simple UDP packets as keep-alive signals.
- Terminates the child process if no signal is received within the timeout.
- Configurable child process path and arguments.
- Configurable UDP listening address and port.
- Configurable timeout duration.
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
- `-t <SECONDS>`, `--timeout-secs <SECONDS>`: Sets the timeout in seconds. If no UDP signal is received for this duration, the child process is terminated.
  - Default: `5`.
- `-h`, `--help`: Prints help information.
- `-V`, `--version`: Prints version information.

**Examples:**

- **Linux/macOS:** Run `sleep 1000`, kill it if no signal received for **10 seconds** (default listener).

  ```bash
  ./target/release/ping-guard -t 10 /usr/bin/sleep 1000
  ```

- **Linux/macOS:** Run `/path/to/my/app --config file.conf`, listening on `127.0.0.1:9999`, default 5-second timeout.

  ```bash
  ./target/release/ping-guard -l 127.0.0.1:9999 /path/to/my/app --config file.conf
  ```

- **Windows:** Run `timeout.exe 1000`, kill it if no signal received for **3 seconds**.

  ```powershell
  .\target\release\ping-guard.exe -t 3 C:\Windows\System32\timeout.exe 1000
  ```

- **Windows:** Run `C:\path\to\app.exe --verbose`, listening on `127.0.0.1:54321` with a **30-second** timeout.
  ```powershell
  .\target\release\ping-guard.exe -l 127.0.0.1:54321 -t 30 C:\path\to\app.exe --verbose
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
  sock.sendto(b'ping', ('127.0.0.1', 12345)) # Replace address/port if needed
  print("Sent UDP ping.")
  ```
- Using TypeScript (Node.js):

  ```typescript
  import dgram from "dgram";

  const message = Buffer.from("ping");
  const client = dgram.createSocket("udp4");
  const targetHost = "127.0.0.1";
  const targetPort = 12345; // Replace if needed

  client.send(message, targetPort, targetHost, (err) => {
    if (err) {
      console.error("Failed to send UDP ping:", err);
    } else {
      console.log(`Sent UDP ping to ${targetHost}:${targetPort}`);
    }
    client.close();
  });
  ```

- Using Rust:

  ```rust
  use std::net::UdpSocket;

  fn main() -> std::io::Result<()> {
      let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available local port
      let target_addr = "127.0.0.1:12345"; // Replace if needed
      socket.send_to(b"ping", target_addr)?;
      println!("Sent UDP ping to {}", target_addr);
      Ok(())
  }
  ```

- Using Ruby:

  ```ruby
  require 'socket'

  message = "ping"
  target_host = '127.0.0.1'
  target_port = 12345 # Replace if needed

  begin
    socket = UDPSocket.new
    socket.send(message, 0, target_host, target_port)
    puts "Sent UDP ping to #{target_host}:#{target_port}"
  rescue => e
    puts "Failed to send UDP ping: #{e.message}"
  ensure
    socket&.close
  end
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
  # Example: Run 'sleep 60' with a 15-second timeout
  cargo run -- -t 15 /usr/bin/sleep 60

  # Example: Listen on a different port, 3s timeout, run 'my_app --test'
  cargo run -- -l 127.0.0.1:8888 -t 3 /path/to/my_app --test
  ```

- **Dependencies:** The project uses `tokio` for asynchronous operations (process handling, networking, timers) and `clap` for command-line argument parsing. Cargo handles dependency management.
- **Formatting and Linting:** Use `cargo fmt` to format the code and `cargo clippy` to check for common mistakes and style issues.
