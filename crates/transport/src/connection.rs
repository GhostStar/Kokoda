use crate::BytesCodec;
use bytes::{Buf, Bytes, BytesMut};
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

fn io_pending(error: &io::Error) -> bool {
    (cfg!(windows) || cfg!(test)) && error.raw_os_error() == Some(997)
}
fn write_pending(error: &io::Error) -> bool {
    io_pending(error)
        || matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
        )
}
use tokio_util::codec::{Decoder, Encoder};

/// Bounded RustDesk byte framing over an authenticated stream.
pub struct Connection<T> {
    stream: T,
    codec: BytesCodec,
    incoming: BytesMut,
    outgoing: BytesMut,
    write_started: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fragmented {
        bytes: std::collections::VecDeque<u8>,
        pause: bool,
    }
    impl Read for Fragmented {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.pause = !self.pause;
            if self.pause {
                return Err(io::ErrorKind::TimedOut.into());
            }
            match self.bytes.pop_front() {
                Some(byte) => {
                    out[0] = byte;
                    Ok(1)
                }
                None => Ok(0),
            }
        }
    }
    impl Write for Fragmented {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn polling_preserves_partial_headers_and_payloads() {
        let payloads = [vec![7; 1024], vec![8; 16]];
        let mut encoded = BytesMut::new();
        let mut codec = BytesCodec::new();
        for payload in &payloads {
            codec
                .encode(Bytes::copy_from_slice(payload), &mut encoded)
                .unwrap();
        }
        let mut connection = Connection::new(Fragmented {
            bytes: encoded.to_vec().into(),
            pause: false,
        });
        for expected in payloads {
            loop {
                match connection.receive() {
                    Ok(actual) => {
                        assert_eq!(&actual[..], &expected);
                        break;
                    }
                    Err(e) => assert_eq!(e.kind(), io::ErrorKind::TimedOut),
                }
            }
        }
    }
    struct PendingIo {
        input: std::collections::VecDeque<u8>,
        sent: Vec<u8>,
        read_pause: bool,
        write_pause: bool,
        flush_pause: bool,
        fatal: bool,
        writes: usize,
    }
    impl PendingIo {
        fn new() -> Self {
            Self {
                input: Default::default(),
                sent: vec![],
                read_pause: false,
                write_pause: false,
                flush_pause: true,
                fatal: false,
                writes: 0,
            }
        }
    }
    impl Read for PendingIo {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.read_pause = !self.read_pause;
            if self.read_pause {
                return Err(io::Error::from_raw_os_error(997));
            }
            match self.input.pop_front() {
                Some(value) => {
                    out[0] = value;
                    Ok(1)
                }
                None => Ok(0),
            }
        }
    }
    impl Write for PendingIo {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.fatal {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.write_pause = !self.write_pause;
            if self.write_pause {
                return Err(io::Error::from_raw_os_error(997));
            }
            let count = data.len().min(7);
            self.sent.extend_from_slice(&data[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.flush_pause {
                self.flush_pause = false;
                return Err(io::Error::from_raw_os_error(997));
            }
            Ok(())
        }
    }
    #[test]
    fn pending_997_preserves_partial_reads_writes_and_flush() {
        let expected = vec![42; 80];
        let mut connection = Connection::new(PendingIo::new());
        connection.send(&expected).unwrap();
        let sent = connection.stream_mut().sent.clone();
        let mut codec = BytesCodec::new();
        let mut data = BytesMut::from(sent.as_slice());
        assert_eq!(&codec.decode(&mut data).unwrap().unwrap()[..], &expected);
        assert!(
            data.is_empty(),
            "A pending write or flush replayed the packet"
        );
        connection.stream_mut().input = sent.into();
        assert_eq!(&connection.receive().unwrap()[..], &expected);
    }
    #[test]
    fn queued_write_yields_on_pending_io_and_preserves_packet_order() {
        let mut c = Connection::new(PendingIo::new());
        c.queue_send(b"video-header").unwrap();
        c.queue_send(&vec![42; 1000]).unwrap();
        c.flush_pending().unwrap();
        assert!(c.has_pending_write());
        c.queue_send(b"control-reply").unwrap();
        for _ in 0..1000 {
            c.flush_pending().unwrap();
            if !c.has_pending_write() {
                break;
            }
        }
        assert!(!c.has_pending_write());
        let mut codec = BytesCodec::new();
        let mut bytes = BytesMut::from(c.stream_mut().sent.as_slice());
        for wanted in [
            b"video-header".to_vec(),
            vec![42; 1000],
            b"control-reply".to_vec(),
        ] {
            assert_eq!(codec.decode(&mut bytes).unwrap().unwrap().as_ref(), wanted);
        }
        assert!(bytes.is_empty());
        assert!(c.queue_send(&vec![0; MAX_PACKET + 1]).is_err());
    }

    #[test]
    fn zero_plaintext_write_recovers_after_flushing_tls_buffer() {
        struct Buffered {
            full: bool,
            bytes: Vec<u8>,
        }
        impl Read for Buffered {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::WouldBlock.into())
            }
        }
        impl Write for Buffered {
            fn write(&mut self, data: &[u8]) -> io::Result<usize> {
                if self.full {
                    return Ok(0);
                }
                self.bytes.extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.full = false;
                Ok(())
            }
        }
        let mut c = Connection::new(Buffered {
            full: true,
            bytes: vec![],
        });
        c.queue_send(b"control").unwrap();
        c.flush_pending().unwrap();
        assert!(!c.has_pending_write());
        let mut bytes = BytesMut::from(c.stream_mut().bytes.as_slice());
        assert_eq!(
            BytesCodec::new()
                .decode(&mut bytes)
                .unwrap()
                .unwrap()
                .as_ref(),
            b"control"
        );
        assert!(bytes.is_empty());
    }

    #[test]
    fn pending_write_is_bounded_and_permanent_errors_fail_immediately() {
        let mut connection = Connection::new(PendingIo::new());
        assert_eq!(
            connection
                .send_bounded(b"test", Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(connection.stream_mut().writes, 1);
        connection.stream_mut().fatal = true;
        assert_eq!(
            connection.send(b"test").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(connection.stream_mut().writes, 2);
    }
}

pub const MAX_PACKET: usize = 16 * 1024 * 1024;

impl<T: Read + Write> Connection<T> {
    pub fn new(stream: T) -> Self {
        let mut codec = BytesCodec::new();
        codec.set_max_packet_length(MAX_PACKET);
        Self {
            stream,
            codec,
            incoming: BytesMut::new(),
            outgoing: BytesMut::new(),
            write_started: None,
        }
    }

    pub fn stream_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Queue complete protocol records without blocking the control reader.
    pub fn queue_send(&mut self, payload: &[u8]) -> io::Result<()> {
        if payload.len() > MAX_PACKET
            || self.outgoing.len() + payload.len() + 8 > MAX_PACKET * 2 + 64
        {
            return Err(io::Error::other("Outgoing queue limit exceeded"));
        }
        self.codec
            .encode(Bytes::copy_from_slice(payload), &mut self.outgoing)?;
        self.write_started.get_or_insert_with(Instant::now);
        Ok(())
    }

    pub fn has_pending_write(&self) -> bool {
        self.write_started.is_some()
    }

    /// One bounded write turn. The caller uses a nonblocking socket and
    /// continues reading input between turns, including while the peer is slow.
    pub fn flush_pending(&mut self) -> io::Result<()> {
        if self.write_started.is_none() {
            return Ok(());
        }
        if self
            .write_started
            .is_some_and(|t| t.elapsed() > Duration::from_secs(10))
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Outgoing queue stalled",
            ));
        }
        if !self.outgoing.is_empty() {
            let count = self.outgoing.len().min(256 * 1024);
            let mut written = self.stream.write(&self.outgoing[..count]);
            // rustls can accept zero plaintext while its TLS buffer is full.
            // A successful flush may free that buffer: retry once before
            // classifying this as a permanent zero-progress writer.
            if matches!(written, Ok(0)) {
                match self.stream.flush() {
                    Err(e) if write_pending(&e) => return Ok(()),
                    Err(e) => return Err(e),
                    Ok(()) => written = self.stream.write(&self.outgoing[..count]),
                }
            }
            match written {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    self.outgoing.advance(n);
                    self.write_started = Some(Instant::now());
                }
                Err(e) if write_pending(&e) => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        if self.outgoing.is_empty() {
            match self.stream.flush() {
                Ok(()) => self.write_started = None,
                Err(e) if write_pending(&e) => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        self.send_bounded(payload, Duration::from_secs(10))
    }
    fn send_bounded(&mut self, payload: &[u8], timeout: Duration) -> io::Result<()> {
        if payload.len() > MAX_PACKET {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Packet exceeds limit",
            ));
        }
        let mut encoded = BytesMut::new();
        self.codec
            .encode(Bytes::copy_from_slice(payload), &mut encoded)?;
        let deadline = Instant::now() + timeout;
        let mut offset = 0;
        while offset < encoded.len() {
            match self.stream.write(&encoded[offset..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Connection write made no progress",
                    ))
                }
                Ok(count) => offset += count,
                Err(error) if write_pending(&error) => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("Sending frame timed out: {error}"),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => return Err(error),
            }
        }
        loop {
            match self.stream.flush() {
                Ok(()) => return Ok(()),
                Err(error) if write_pending(&error) => {
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("Flushing frame timed out: {error}"),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub fn receive(&mut self) -> io::Result<BytesMut> {
        let pending_deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(frame) = self.codec.decode(&mut self.incoming)? {
                return Ok(frame);
            }
            let mut chunk = [0; 16384];
            let count = match self.stream.read(&mut chunk) {
                Ok(count) => count,
                Err(error) if io_pending(&error) || error.kind() == io::ErrorKind::Interrupted => {
                    if Instant::now() >= pending_deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("Pending receive timed out: {error}"),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Connection closed",
                ));
            }
            self.incoming.extend_from_slice(&chunk[..count]);
        }
    }
}
