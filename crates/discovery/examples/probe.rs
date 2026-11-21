//! Isolated LAN fixture: only print endpoints matching the explicit synthetic name.
use std::time::{Duration, Instant};
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("announce") => {
            let _a = kokoda_discovery::Advertiser::new(&args[2], args[3].parse()?)?;
            println!("READY");
            std::thread::sleep(Duration::from_secs(args[4].parse()?));
        }
        Some("find") => {
            let mut b = kokoda_discovery::Browser::new()?;
            let until = Instant::now() + Duration::from_secs(args[3].parse()?);
            while Instant::now() < until {
                if let Some(host) = b.snapshot().into_iter().find(|h| h.name == args[2]) {
                    println!("{}", host.addresses.join(" "));
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            return Err("Synthetic discovery fixture not found".into());
        }
        Some("lifecycle") => {
            let name = format!("Kokoda-Discovery-Test-{}", std::process::id());
            let mut b = kokoda_discovery::Browser::new()?;
            let a = kokoda_discovery::Advertiser::new(&name, 47832)?;
            let until = Instant::now() + Duration::from_secs(12);
            let mut found = false;
            while Instant::now() < until {
                if b.snapshot().iter().any(|h| h.name == name) {
                    found = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !found {
                return Err("Synthetic announcement not discovered".into());
            }
            drop(a);
            let until = Instant::now() + Duration::from_secs(12);
            while Instant::now() < until {
                if !b.snapshot().iter().any(|h| h.name == name) {
                    println!("PASS discovery announcement and goodbye removal");
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            return Err("Stopped synthetic host remained in discovery".into());
        }
        _ => return Err("Use announce NAME PORT SECONDS | find NAME SECONDS | lifecycle".into()),
    }
    Ok(())
}
