# cattysend

`cattysend` is a high-performance, Rust-based implementation of the **Mutual Transfer Alliance (MTA)** protocol, specifically designed for Linux terminal environments. It enables seamless, high-speed file discovery and transfer between Linux desktops and mobile devices (Xiaomi, OPPO, vivo, etc.) using Bluetooth Low Energy (BLE) and Wi-Fi Direct (P2P).

## Origins and Lineage

This project is heavily inspired by [CatShare](https://github.com/kmod-midori/CatShare), a pioneering implementation of the MTA protocol. `cattysend` aims to extend this lineage by providing a native Linux TUI experience, optimized for headless servers and developer workflows.

The underlying protocol is a reverse-engineered implementation of the standards used by the Mutual Transfer Alliance. It is not an official implementation, but it adheres to the cryptographic and transport specifications required for cross-device interoperability.

## Implementation Status

The project is currently in an active **Work in Progress (WIP)** state. While the core engine is operational, a fully stable binary release is pending.

### Feature Matrix

| Module | Feature | Status | Notes |
| :--- | :--- | :--- | :--- |
| **Discovery** | BLE GATT Advertisement | ✅ Done | Requires BlueZ |
| **Security** | ECDH (P-256) Key Exchange | ✅ Done | Native implementation |
| **Transport** | Wi-Fi Direct (P2P) | ✅ Done | Managed via NetworkManager |
| **Interface** | CLI Frontend | 🚧 Alpha | Basic commands working |
| **Interface** | TUI Frontend | 🚧 Alpha | Interactive selection pending |

## Technical Architecture & Constraints

### The "Sudo-less" Philosophy
A primary design goal of `cattysend` is to maintain system integrity. Unlike many Linux networking tools that require `CAP_NET_ADMIN` or `sudo` for raw socket manipulation, `cattysend` delegates all network operations to the **NetworkManager (NM)** daemon via its D-Bus interface.

### The Connectivity Trade-off
Current Linux desktop infrastructure presents a significant challenge for concurrent Wi-Fi operations. While modern wireless hardware typically supports multiple concurrent interfaces (e.g., Managed + P2P-Client), the NetworkManager policy engine often lacks the logic to parse `NL80211_ATTR_INTERFACE_COMBINATIONS` from the kernel.

**Current limitation:** 
When activating a P2P connection, `cattysend` uses the native `nmcli` backend. Due to upstream NM implementation details, the physical Wi-Fi interface may temporarily suspend its infrastructure connection to prioritize the P2P group. We have chosen this "preemptive" behavior as a safer, more robust alternative to injecting unmanaged `wpa_supplicant` instances or requiring insecure `sudoers` configurations.

## Building from Source

To build `cattysend`, you need a functional Rust toolchain and the development headers for D-Bus and BlueZ.

### Dependencies
- `libdbus-1-dev` (or equivalent)
- `libbluetooth-dev` (BlueZ)
- `NetworkManager` (Runtime)

### Build Command
```bash
cargo build --release
```

The resulting binaries will be located in `target/release/`:
- `cattysend-core`: Core library
- `cattysend-tui`: The terminal user interface (recommended)
- `cattysend-cli`: Command line utility

## Acknowledgments

Deep gratitude to the developers of **CatShare** for their initial research into the MTA protocol. This project serves as a complementary implementation for the Linux community.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
