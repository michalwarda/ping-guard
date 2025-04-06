.PHONY: all clean build-linux build-windows build-macos

all: build-linux build-windows build-macos

# Build for Linux (x86_64)
build-linux:
	@echo "Building for Linux (x86_64-unknown-linux-gnu)..."
	cargo build --release --target x86_64-unknown-linux-gnu
	@mkdir -p builds
	@cp target/x86_64-unknown-linux-gnu/release/ping-guard builds/ping-guard-linux-x86_64
	@echo "Linux build complete: builds/ping-guard-linux-x86_64"

# Build for Windows (x86_64)
build-windows:
	@echo "Building for Windows (x86_64-pc-windows-msvc)..."
	cargo build --release --target x86_64-pc-windows-msvc
	@mkdir -p builds
	@cp target/x86_64-pc-windows-msvc/release/ping-guard.exe builds/ping-guard-windows-x86_64.exe
	@echo "Windows build complete: builds/ping-guard-windows-x86_64.exe"

# Build for macOS (Apple Silicon)
build-macos:
	@echo "Building for macOS (aarch64-apple-darwin)..."
	cargo build --release --target aarch64-apple-darwin
	@mkdir -p builds
	@cp target/aarch64-apple-darwin/release/ping-guard builds/ping-guard-macos-aarch64
	@echo "macOS build complete: builds/ping-guard-macos-aarch64"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@rm -rf builds
	@echo "Clean complete"

# Cross-compilation using 'cross' tool
cross-all:
	@echo "Building for all platforms using cross tool..."
	@command -v cross >/dev/null 2>&1 || { echo "Error: 'cross' is not installed. Install with 'cargo install cross'."; exit 1; }
	@mkdir -p builds
	cross build --release --target x86_64-unknown-linux-gnu
	@cp target/x86_64-unknown-linux-gnu/release/ping-guard builds/ping-guard-linux-x86_64
	cross build --release --target x86_64-pc-windows-msvc || echo "Warning: Windows build failed with cross. Try native Windows build."
	@cp target/x86_64-pc-windows-msvc/release/ping-guard.exe builds/ping-guard-windows-x86_64.exe 2>/dev/null || echo "No Windows binary produced."
	cross build --release --target aarch64-apple-darwin || echo "Warning: macOS build failed with cross. Try native macOS build."
	@cp target/aarch64-apple-darwin/release/ping-guard builds/ping-guard-macos-aarch64 2>/dev/null || echo "No macOS binary produced."
	@echo "Cross compilation complete. Available binaries in 'builds/' directory." 