use base64::{engine::general_purpose::STANDARD, Engine};
use kokoda_transport::tls;
use std::io::{self, BufRead, Write};

fn exchange(connection: &mut tls::Client, request: &[u8]) -> tls::Result<serde_json::Value> {
    // Validate locally before sending user input to the remote protocol.
    let value: serde_json::Value = serde_json::from_slice(request)?;
    kokoda_protocol::validate(&value).map_err(std::io::Error::other)?;
    let started = std::time::Instant::now();
    connection.send(request)?;
    let response = connection.receive()?;
    let mut json: serde_json::Value = serde_json::from_slice(&response)?;
    if json.get("type").and_then(|v| v.as_str()) == Some("frame") {
        json["image"] = STANDARD.encode(connection.receive()?).into();
    }
    json["round_trip_ms"] = started.elapsed().as_secs_f64().mul_add(1000.0, 0.0).into();
    Ok(json)
}

// One network owner keeps TLS records ordered while receiving unsolicited video.
// A bounded stdin queue prevents control requests accumulating without limit.
fn stream_bridge(connection: &mut tls::Client) -> tls::Result<()> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::sync_channel::<String>(32);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    stream_bridge_io(connection, rx, &mut io::stdout().lock())
}

fn stream_bridge_io(
    connection: &mut tls::Client,
    rx: std::sync::mpsc::Receiver<String>,
    output: &mut impl Write,
) -> tls::Result<()> {
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    connection.stream_mut().sock.set_nonblocking(true)?;
    let mut pending = std::collections::VecDeque::new();
    let mut frame_header: Option<serde_json::Value> = None;
    loop {
        if pending
            .front()
            .is_some_and(|(t, _, _): &(Instant, bool, u64)| t.elapsed() > Duration::from_secs(30))
        {
            return Err(
                io::Error::new(io::ErrorKind::TimedOut, "Control response timed out").into(),
            );
        }
        connection.flush_pending()?;
        // Bounded pipelining: input never waits for an application response.
        // FIFO replies stay correlated even when unsolicited frames interleave.
        if pending.len() < 32 && !connection.has_pending_write() {
            match rx.try_recv() {
                Ok(line) => {
                    let mut value: serde_json::Value = serde_json::from_str(&line)?;
                    kokoda_protocol::validate(&value).map_err(std::io::Error::other)?;
                    let asynchronous = value
                        .get("bridge_async")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if asynchronous
                        && !matches!(value["command"].as_str(), Some("input" | "video_feedback"))
                    {
                        return Err(io::Error::other("Invalid asynchronous command").into());
                    }
                    value.as_object_mut().unwrap().remove("bridge_async");
                    let sid = value["session_id"].as_u64().unwrap_or(0);
                    connection.queue_send(&serde_json::to_vec(&value)?)?;
                    pending.push_back((Instant::now(), asynchronous, sid));
                    connection.flush_pending()?;
                }
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let packet = match connection.receive() {
            Ok(packet) => packet,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let mut json = if let Some(mut header) = frame_header.take() {
            header["image"] = STANDARD.encode(packet).into();
            header
        } else {
            let header: serde_json::Value = serde_json::from_slice(&packet)?;
            if header["type"] == "frame" {
                frame_header = Some(header);
                continue;
            }
            header
        };
        if json["event"] != true {
            if let Some((start, asynchronous, sid)) = pending.pop_front() {
                json["bridge_async"] = asynchronous.into();
                if json.get("session_id").is_none() {
                    json["session_id"] = sid.into();
                }
                json["round_trip_ms"] = (start.elapsed().as_secs_f64() * 1000.0).into();
            }
        }
        if json["type"] == "windows" {
            json["bridge_pipeline"] = true.into();
        }
        serde_json::to_writer(&mut *output, &json)?;
        writeln!(output)?;
        output.flush()?;
    }
}

fn run(phase: &mut &str) -> tls::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2
        || args.len() > 3
        || (args.len() == 3 && !matches!(args[2].as_str(), "--bridge" | "--stream-bridge"))
    {
        return Err(
            "Usage: kokoda-bridge <host:port> --password-stdin [--bridge | --stream-bridge]".into(),
        );
    }
    *phase = "connect";
    let mut connection = if args[1] == "--password-stdin" {
        let password = kokoda_transport::password::read_secret(&mut io::stdin().lock())?;
        kokoda_transport::password::connect(&args[0], &password)?
    } else {
        tls::connect(&args[0], args[1].as_ref())?
    };
    *phase = "session";
    if args.get(2).is_some_and(|a| a == "--stream-bridge") {
        return stream_bridge(&mut connection);
    }
    if args.len() == 3 {
        let stdin = io::stdin();
        let mut output = io::stdout().lock();
        for request in stdin.lock().lines() {
            let response = exchange(&mut connection, request?.as_bytes())?;
            serde_json::to_writer(&mut output, &response)?;
            writeln!(output)?;
            output.flush()?;
        }
    } else {
        let json = exchange(&mut connection, br#"{"command":"list_windows"}"#)?;
        println!("{}", serde_json::to_string_pretty(&json)?);
    }
    Ok(())
}

fn failure_category(
    error: &(dyn std::error::Error + Send + Sync + 'static),
    phase: &str,
) -> (&'static str, bool) {
    if let Some(io) = error.downcast_ref::<io::Error>() {
        use io::ErrorKind::*;
        return match io.kind() {
            NotFound => ("pairing", false),
            PermissionDenied if phase == "connect" && io.to_string() == "Authentication failed" => {
                ("authentication", false)
            }
            PermissionDenied => ("permission", false),
            ConnectionRefused | HostUnreachable | NetworkUnreachable | TimedOut | WouldBlock
            | ConnectionReset | ConnectionAborted | BrokenPipe => ("network", true),
            // EOF before the greeting is a transport interruption, not evidence of bad credentials.
            UnexpectedEof => ("network", true),
            _ if phase == "connect" => ("authentication", false),
            _ => ("protocol", false),
        };
    }
    if phase == "connect" {
        ("authentication", false)
    } else {
        ("protocol", false)
    }
}
fn main() {
    let mut phase = "configuration";
    if let Err(error) = run(&mut phase) {
        let (category, retryable) = failure_category(error.as_ref(), phase);
        if std::env::args().any(|arg| matches!(arg.as_str(), "--bridge" | "--stream-bridge")) {
            println!(
                "{}",
                serde_json::json!({"type":"bridge_error","category":category,"retryable":retryable,"message":error.to_string()})
            );
        }
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_sends_before_first_reply_and_correlates_interleaved_video() {
        use std::{
            net::TcpListener,
            sync::mpsc,
            time::{Duration, SystemTime, UNIX_EPOCH},
        };
        let path = std::env::temp_dir().join(format!(
            "kokoda-pipeline-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        tls::initialize(&path).unwrap();
        let config = tls::server_config(&path).unwrap();
        let token = tls::pairing_token(&path).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            let mut c = tls::accept(listener.accept().unwrap().0, config, &token).unwrap();
            c.stream_mut()
                .sock
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            // No reply until all three requests arrive: stop-and-wait deadlocks here.
            for _ in 0..3 {
                let v: serde_json::Value = serde_json::from_slice(&c.receive().unwrap()).unwrap();
                assert!(v.get("bridge_async").is_none());
            }
            c.send(br#"{"type":"frame","event":true,"session_id":1}"#)
                .unwrap();
            c.send(b"synthetic").unwrap();
            for _ in 0..3 {
                c.send(br#"{"type":"input_ok","session_id":1}"#).unwrap();
            }
        });
        let mut c = tls::connect(&address, &path).unwrap();
        let (tx, rx) = mpsc::sync_channel(4);
        for asynchronous in [true, false, true] {
            tx.send(serde_json::json!({"command":"input","session_id":1,"events":[{"kind":"release"}],"bridge_async":asynchronous}).to_string()).unwrap();
        }
        let mut output = Vec::new();
        let result = stream_bridge_io(&mut c, rx, &mut output);
        assert!(result.is_err());
        server.join().unwrap();
        drop(tx);
        std::fs::remove_dir_all(&path).unwrap();
        let replies: Vec<serde_json::Value> = output
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_slice(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 4);
        assert_eq!(replies[0]["event"], true);
        for (reply, wanted) in replies[1..].iter().zip([true, false, true]) {
            assert_eq!(reply["bridge_async"], wanted);
            assert!(reply["round_trip_ms"].is_number());
        }
    }

    #[test]
    fn authentication_and_local_permissions_do_not_retry() {
        assert_eq!(
            failure_category(
                &io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed"),
                "connect"
            ),
            ("authentication", false)
        );
        assert_eq!(
            failure_category(&io::Error::from(io::ErrorKind::UnexpectedEof), "connect"),
            ("network", true)
        );
        assert_eq!(
            failure_category(&io::Error::from(io::ErrorKind::UnexpectedEof), "session"),
            ("network", true)
        );
        assert_eq!(
            failure_category(&io::Error::from(io::ErrorKind::PermissionDenied), "connect"),
            ("permission", false)
        );
        assert_eq!(
            failure_category(
                &io::Error::from(io::ErrorKind::ConnectionRefused),
                "connect"
            ),
            ("network", true)
        );
    }
}
