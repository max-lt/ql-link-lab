// SPDX-FileCopyrightText: 2026 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stream the device's debug log to stdout: a headless keyos-log-viewer, so device-side CCID/PIV
//! activity can be captured next to the host-side `fl` harnesses. The debug interface is held
//! exclusively, so close keyos-log-viewer before running this.
//!
//! Wire format (device -> host) mirrors os/usb-debug/protocol: bulk-IN frames `[TYPE:1][PAYLOAD]`
//! ended by a short packet; TYPE 0x01 (Log) carries 0x1E-terminated log records.

use std::{
    io::Write,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rusb::{DeviceHandle, Direction, GlobalContext, TransferType};

const PASSPORT_VID: u16 = 0x1307;
const PASSPORT_PID: u16 = 0x0165;
const FRAME_LOG: u8 = 0x01;
const RECORD_TERMINATOR: u8 = 0x1e;
const READ_CHUNK: usize = 64 * 1024;
const RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(clap::Args)]
pub struct Args {
    /// Only print log records containing this substring (e.g. "ccid").
    #[arg(long)]
    filter: Option<String>,
    /// Stop after printing this many records.
    #[arg(long)]
    limit: Option<usize>,
    /// Stop after this many seconds.
    #[arg(long)]
    timeout: Option<u64>,
}

pub fn run(args: Args) -> Result<()> {
    let deadline = args.timeout.map(|secs| Instant::now() + Duration::from_secs(secs));
    let mut printed = 0usize;
    let mut records: Vec<u8> = Vec::new();
    let stdout = std::io::stdout();
    let mut waiting = false;

    'reconnect: loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Ok(());
        }

        let (handle, ep_in, iface) = match connect() {
            Ok(c) => c,
            Err(_) => {
                if !waiting {
                    eprintln!("waiting for Passport Prime (1307:0165) to be plugged in + running...");
                    waiting = true;
                }
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
        };
        waiting = false;
        eprintln!("streaming device logs (Ctrl-C to stop)");
        records.clear();

        let mut buf = vec![0u8; READ_CHUNK];
        let mut frame: Vec<u8> = Vec::new();
        loop {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                return Ok(());
            }
            let end_of_frame = match handle.read_bulk(ep_in, &mut buf, Duration::from_millis(200)) {
                Ok(0) => true,
                Ok(n) => {
                    frame.extend_from_slice(&buf[..n]);
                    n < READ_CHUNK
                }
                Err(rusb::Error::Timeout) => continue,
                Err(_) => {
                    // Device went away (unplug/reboot); drop the handle and wait for it to return.
                    let _ = handle.release_interface(iface);
                    eprintln!("device disconnected, waiting for it to come back...");
                    continue 'reconnect;
                }
            };
            if !end_of_frame || frame.is_empty() {
                continue;
            }

            // A complete frame: TYPE byte then payload. Only Log frames carry records.
            if frame[0] == FRAME_LOG {
                records.extend_from_slice(&frame[1..]);
                while let Some(pos) = records.iter().position(|b| *b == RECORD_TERMINATOR) {
                    let line = String::from_utf8_lossy(&records[..pos])
                        .trim_end_matches(['\r', '\n'])
                        .to_string();
                    records.drain(..=pos);
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(want) = &args.filter {
                        if !line.contains(want.as_str()) {
                            continue;
                        }
                    }
                    let mut out = stdout.lock();
                    let _ = writeln!(out, "{line}");
                    drop(out);
                    printed += 1;
                    if args.limit.is_some_and(|n| printed >= n) {
                        return Ok(());
                    }
                }
            }
            frame.clear();
        }
    }
}

/// Open the Prime and claim its vendor-specific (class 0xFF) debug interface's bulk-IN endpoint.
fn connect() -> Result<(DeviceHandle<GlobalContext>, u8, u8)> {
    // `mut` is needed for detach/claim on some rusb versions, unused on others.
    #[allow(unused_mut)]
    let mut handle = rusb::open_device_with_vid_pid(PASSPORT_VID, PASSPORT_PID)
        .context("no Passport Prime (1307:0165)")?;
    let config = handle.device().active_config_descriptor().context("reading config descriptor")?;

    let mut iface = None;
    let mut ep_in = None;
    for interface in config.interfaces() {
        for desc in interface.descriptors() {
            if desc.class_code() != 0xff {
                continue;
            }
            for ep in desc.endpoint_descriptors() {
                if ep.transfer_type() == TransferType::Bulk && ep.direction() == Direction::In {
                    iface = Some(desc.interface_number());
                    ep_in = Some(ep.address());
                }
            }
        }
    }
    let iface = iface.context("vendor debug interface (class 0xFF) not found")?;
    let ep_in = ep_in.context("debug bulk-IN endpoint not found")?;

    let _ = handle.detach_kernel_driver(iface);
    handle
        .claim_interface(iface)
        .context("claiming the debug interface (close keyos-log-viewer first -- it holds it)")?;
    Ok((handle, ep_in, iface))
}
