#[cfg(windows)]
use kokoda_host::windows::server;
#[cfg(windows)]
use kokoda_platform_windows::{capture, ffmpeg_encoder, runtime, windows};
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _runtime = runtime::Apartment::new()?;
    use kokoda_app_model::application_windows;
    use windows::enumerate;
    windows::initialize_dpi();
    let windows = enumerate()?;
    let mut args = std::env::args().skip(1);
    if let Some(selected) = args.next() {
        if selected == "--set-password-stdin" {
            let file = args.next().ok_or("Expected password record file")?;
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            let password = kokoda_transport::password::read_secret(&mut std::io::stdin().lock())?;
            let record = kokoda_transport::password::enroll(&password)?;
            kokoda_platform_windows::credentials::save(file.as_ref(), &record)?;
            return Ok(());
        }
        if selected == "--init" {
            let directory = args.next().ok_or("Expected identity directory")?;
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            kokoda_transport::tls::initialize(directory.as_ref())?;
            return Ok(());
        }
        if selected == "--probe-nvenc"
            || selected == "--probe-codecs"
            || selected == "--probe-ffmpeg-software"
        {
            let names: &[&str] = if selected == "--probe-nvenc" {
                &["h264_nvenc"]
            } else if selected == "--probe-ffmpeg-software" {
                &["h264_mf"]
            } else {
                &["h264_nvenc", "h264_amf", "h264_qsv"]
            };
            // Optional Annex B output for single-encoder diagnostics, allowing
            // an independent decoder to verify packets without capturing a desktop.
            let output_path = if names.len() == 1 { args.next() } else { None };
            if args.next().is_some() {
                return Err("Unexpected probe arguments".into());
            }
            let mut output = output_path.map(std::fs::File::create).transpose()?;
            let mut successes = 0;
            for &name in names {
                let mut probe = || -> kokoda_transport::tls::Result<usize> {
                    let mut encoder =
                        ffmpeg_encoder::Encoder::named(name, (1280, 720), 60, 8_000_000)?;
                    let mut pixels = vec![16; 1280 * 720 * 3 / 2];
                    pixels[1280 * 720..].fill(128);
                    let mut bytes = 0;
                    for frame in 0..60 {
                        pixels[..1280 * 720].fill(16 + (frame % 200) as u8);
                        if let Some(packet) = encoder.encode(&pixels)? {
                            bytes += packet.len();
                            if let Some(file) = output.as_mut() {
                                use std::io::Write;
                                file.write_all(&packet)?;
                            }
                        }
                    }
                    if let Some(packet) = encoder.finish()? {
                        bytes += packet.len();
                        if let Some(file) = output.as_mut() {
                            use std::io::Write;
                            file.write_all(&packet)?;
                        }
                    }
                    if bytes == 0 {
                        return Err("Encoder produced no packets".into());
                    }
                    Ok(bytes)
                };
                match probe() {
                    Ok(bytes) => {
                        successes += 1;
                        println!(
                            "{}",
                            serde_json::json!({"encoder":name,"ok":true,"submitted_frames":60,"bytes":bytes})
                        );
                    }
                    Err(error) => println!(
                        "{}",
                        serde_json::json!({"encoder":name,"ok":false,"error":error.to_string()})
                    ),
                }
            }
            if successes == 0 {
                return Err("No requested FFmpeg encoder passed probe".into());
            }
            return Ok(());
        }
        if selected == "--serve-development" || selected == "--serve-password" {
            let address = args.next().ok_or("Expected listen address")?;
            let directory = args.next().ok_or("Expected identity directory")?;
            use kokoda_host::windows::sharing::Sharing;
            let sharing = match args.next().as_deref() {
                None => Sharing::default(),
                Some("--allow-all-windows") => Sharing::all_development(),
                Some("--sharing-policy") => {
                    Sharing::load(args.next().ok_or("Expected policy file")?.as_ref())?
                }
                _ => return Err("Expected --sharing-policy FILE or --allow-all-windows".into()),
            };
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            eprintln!(
                "{}",
                if sharing.restricted() {
                    "DEVELOPMENT MODE: selected-window sharing is read-only; input and clipboard are disabled."
                } else {
                    "DEVELOPMENT MODE: paired controllers can access all application windows."
                }
            );
            return if selected == "--serve-password" {
                server::serve_password(&address, directory.as_ref(), std::sync::Arc::new(sharing))
            } else {
                server::serve(&address, directory.as_ref(), std::sync::Arc::new(sharing))
            };
        }
        if selected == "--init-policy" {
            use std::io::Write;
            let path = args.next().ok_or("Expected policy file")?;
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(
                serde_json::to_string_pretty(&kokoda_host::sharing::Policy::default())?.as_bytes(),
            )?;
            return Ok(());
        }
        if selected == "--preview-policy" {
            let path = args.next().ok_or("Expected policy file")?;
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            let sharing = kokoda_host::windows::sharing::Sharing::load(path.as_ref())?;
            println!("{}", serde_json::to_string_pretty(&sharing.catalog()?)?);
            return Ok(());
        }
        if selected == "--snapshot" {
            let id = args.next().ok_or("Expected window ID")?.parse::<u64>()?;
            let path = args.next().ok_or("Expected output PNG path")?;
            if args.next().is_some() {
                return Err("Unexpected arguments".into());
            }
            capture::snapshot(id, path.into())?;
            println!("Window snapshot saved.");
            return Ok(());
        }
        let selected = selected.parse::<u64>()?;
        let processes = args
            .map(|arg| arg.parse::<u32>())
            .collect::<Result<Vec<_>, _>>()?;
        println!(
            "{}",
            serde_json::to_string_pretty(&application_windows(&windows, selected, &processes)?)?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&windows)?);
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Window discovery requires Windows.");
    std::process::exit(1);
}
