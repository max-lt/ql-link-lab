// SPDX-FileCopyrightText: 2026 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drive the device's FIDO interface over CTAPHID. For now this only locates the
//! HID interface; the U2F/CTAP2 framing will be lifted from passport-drive.

use anyhow::{bail, Context, Result};

const PASSPORT_VID: u16 = 0x1307;
const PASSPORT_PID: u16 = 0x0165;

#[derive(clap::Args)]
pub struct Args {
    /// Run against the hosted simulator instead of a real device (not wired up yet).
    #[arg(long)]
    hosted: bool,
}

pub fn run(args: Args) -> Result<()> {
    if args.hosted {
        bail!("--hosted is not wired up yet (the sim has no HID transport)");
    }

    let api = hidapi::HidApi::new().context("open the HID API")?;
    let device = api
        .device_list()
        .find(|d| d.vendor_id() == PASSPORT_VID && d.product_id() == PASSPORT_PID)
        .with_context(|| format!("no Passport Prime HID device ({PASSPORT_VID:04x}:{PASSPORT_PID:04x})"))?;

    println!(
        "found FIDO HID device {:04x}:{:04x} -- {}",
        device.vendor_id(),
        device.product_id(),
        device.product_string().unwrap_or("")
    );
    println!("fido: U2F/CTAP2 driving is TODO (reuse passport-drive's CTAPHID framing).");
    Ok(())
}
