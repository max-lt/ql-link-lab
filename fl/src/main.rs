// SPDX-FileCopyrightText: 2026 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Foundation Lab CLI: one entry point for the device and QL test harnesses.
//! Each mode is feature-gated; `all` is on by default.

use clap::{Parser, Subcommand};

#[cfg(feature = "fido")]
mod fido;
#[cfg(feature = "piv")]
mod piv;

#[derive(Parser)]
#[command(name = "fl", about = "Foundation Lab -- device and QL test harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// QLRP opaque byte relay (sim <-> backend).
    #[cfg(feature = "relay")]
    Relay,
    /// QL2 initiator: XX/IK handshake, ql-rpc routes, optional MCP.
    #[cfg(feature = "backend")]
    Backend {
        /// Arguments forwarded to the backend, e.g. -- --serve --token <hex>.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Drive the device FIDO interface (U2F/CTAP2) over HID.
    #[cfg(feature = "fido")]
    Fido(fido::Args),
    /// Drive the device PIV interface over CCID/PCSC.
    #[cfg(feature = "piv")]
    Piv(piv::Args),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        #[cfg(feature = "relay")]
        Command::Relay => block_on(relay::run()),
        #[cfg(feature = "backend")]
        Command::Backend { args } => block_on(backend::run(args)),
        #[cfg(feature = "fido")]
        Command::Fido(args) => fido::run(args)?,
        #[cfg(feature = "piv")]
        Command::Piv(args) => piv::run(args)?,
    }
    Ok(())
}

#[cfg(any(feature = "relay", feature = "backend"))]
fn block_on<F: std::future::Future<Output = ()>>(fut: F) {
    tokio::runtime::Runtime::new().expect("build tokio runtime").block_on(fut)
}
