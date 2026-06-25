// SPDX-FileCopyrightText: 2026 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dump and decode the USB descriptors of CCID smart-card readers so two devices (our Passport
//! Prime vs a reference reader like a YubiKey) can be compared at the descriptor level. Userspace
//! USB sniffing is not possible on macOS, but the differences that matter -- the interrupt
//! endpoint, dwFeatures, slot count -- live in the descriptors, not the traffic.

use anyhow::{Context, Result};
use nusb::{descriptors::TransferType, MaybeFuture};

const CCID_CLASS: u8 = 0x0b;
const CCID_FUNCTIONAL: u8 = 0x21;

#[derive(clap::Args)]
pub struct Args {
    /// Only show devices whose product string contains this (case-insensitive), e.g. "Yubi".
    #[arg(long)]
    filter: Option<String>,
    /// Show every interface, not just CCID smart-card readers.
    #[arg(long)]
    all: bool,
}

pub fn run(args: Args) -> Result<()> {
    for info in nusb::list_devices().wait().context("list USB devices")? {
        let product = info.product_string().unwrap_or("<unknown>").to_string();
        if let Some(f) = &args.filter {
            if !product.to_lowercase().contains(&f.to_lowercase()) {
                continue;
            }
        }

        let dev = match info.open().wait() {
            Ok(d) => d,
            Err(e) => {
                println!("{:04x}:{:04x}  {product}  (cannot open: {e})", info.vendor_id(), info.product_id());
                continue;
            }
        };

        let mut header = false;
        for config in dev.configurations() {
            for alt in config.interface_alt_settings() {
                let ccid = alt.class() == CCID_CLASS;
                if !ccid && !args.all {
                    continue;
                }
                if !header {
                    println!("\n{:04x}:{:04x}  {product}", info.vendor_id(), info.product_id());
                    header = true;
                }
                println!(
                    "  interface {} class {:#04x}/{:#04x}/{:#04x}{}",
                    alt.interface_number(),
                    alt.class(),
                    alt.subclass(),
                    alt.protocol(),
                    if ccid { "  (CCID smart-card reader)" } else { "" }
                );
                for ep in alt.endpoints() {
                    let dir = if ep.address() & 0x80 != 0 { "IN " } else { "OUT" };
                    println!(
                        "    ep {:#04x} {:<11} {dir} maxpkt {:<4} interval {}",
                        ep.address(),
                        transfer_type(ep.transfer_type()),
                        ep.max_packet_size(),
                        ep.interval(),
                    );
                }
                if ccid {
                    for desc in alt.descriptors() {
                        if desc.descriptor_type() == CCID_FUNCTIONAL {
                            print_ccid_functional(&desc);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn transfer_type(t: TransferType) -> &'static str {
    match t {
        TransferType::Control => "Control",
        TransferType::Isochronous => "Isochronous",
        TransferType::Bulk => "Bulk",
        TransferType::Interrupt => "Interrupt",
    }
}

/// Decode the CCID functional descriptor fields that drive host behaviour.
fn print_ccid_functional(d: &[u8]) {
    if d.len() < 48 {
        println!("    CCID functional descriptor: truncated ({} bytes)", d.len());
        return;
    }
    let le32 = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    println!("    CCID functional descriptor (0x21):");
    println!("      bMaxSlotIndex          = {}", d[4]);
    println!("      bVoltageSupport        = {:#04x}", d[5]);
    println!("      dwProtocols            = {:#010x}", le32(6));
    println!("      dwFeatures             = {:#010x}", le32(40));
    println!("      dwMaxCCIDMessageLength = {}", le32(44));
}
