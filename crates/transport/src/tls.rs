use crate::connection::Connection;
use rcgen::generate_simple_self_signed;
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName},
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};
use std::{error::Error, fs, io, net::TcpStream, path::Path, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub type Client = Connection<StreamOwned<ClientConnection, TcpStream>>;
pub type Server = Connection<StreamOwned<ServerConnection, TcpStream>>;

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Initialize only a new directory; an existing identity is never overwritten.
pub fn initialize(directory: &Path) -> Result<()> {
    let identity = generate_simple_self_signed(vec!["kokoda.local".into()])?;
    let mut token = [0u8; 32];
    provider()
        .secure_random
        .fill(&mut token)
        .map_err(|_| io::Error::other("Random generation failed"))?;
    fs::create_dir(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    }
    fs::write(directory.join("certificate.der"), identity.cert.der())?;
    fs::write(
        directory.join("private-key.der"),
        identity.key_pair.serialize_der(),
    )?;
    fs::write(directory.join("pairing-token"), token)?;
    Ok(())
}

pub fn server_config(directory: &Path) -> Result<Arc<ServerConfig>> {
    let certificate = CertificateDer::from(fs::read(directory.join("certificate.der"))?);
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(fs::read(
        directory.join("private-key.der"),
    )?));
    Ok(Arc::new(
        ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)?,
    ))
}

pub fn pairing_token(directory: &Path) -> Result<[u8; 32]> {
    fs::read(directory.join("pairing-token"))?
        .try_into()
        .map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid pairing token length").into()
        })
}

pub fn accept(socket: TcpStream, config: Arc<ServerConfig>, token: &[u8; 32]) -> Result<Server> {
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    socket.set_nodelay(true)?;
    let mut connection = Connection::new(StreamOwned::new(ServerConnection::new(config)?, socket));
    let supplied = connection.receive()?;
    if !bool::from(supplied.as_ref().ct_eq(token)) {
        connection.send(kokoda_protocol::AUTHENTICATION_FAILED)?;
        return Err(
            io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed").into(),
        );
    }
    connection.send(kokoda_protocol::VERSION.as_bytes())?;
    Ok(connection)
}

// Retry only TCP establishment: authenticated commands and TLS failures are never replayed.
pub(crate) fn retry_connect<T>(
    mut attempt: impl FnMut() -> io::Result<T>,
    mut pause: impl FnMut(Duration),
) -> io::Result<T> {
    for retry in 0..3 {
        match attempt() {
            Ok(value) => return Ok(value),
            Err(error) => {
                let transient = matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::HostUnreachable
                        | io::ErrorKind::NetworkUnreachable
                        | io::ErrorKind::Interrupted
                );
                if !transient || retry == 2 {
                    return Err(error);
                }
                pause(Duration::from_millis(500 * (retry + 1)));
            }
        }
    }
    unreachable!()
}

/// The certificate is supplied out of band during pairing; normal TLS validation remains enabled.
pub fn connect(address: &str, directory: &Path) -> Result<Client> {
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(fs::read(
        directory.join("certificate.der"),
    )?))?;
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    use std::net::ToSocketAddrs;
    let endpoint = address
        .to_socket_addrs()?
        .next()
        .ok_or("No server address")?;
    let socket = retry_connect(
        || TcpStream::connect_timeout(&endpoint, Duration::from_secs(5)),
        std::thread::sleep,
    )?;
    socket.set_read_timeout(Some(Duration::from_secs(15)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    socket.set_nodelay(true)?;
    let mut connection = Connection::new(StreamOwned::new(
        ClientConnection::new(Arc::new(config), ServerName::try_from("kokoda.local")?)?,
        socket,
    ));
    connection.send(&pairing_token(directory)?)?;
    let greeting = connection.receive()?;
    if greeting.as_ref() == kokoda_protocol::AUTHENTICATION_FAILED {
        return Err(
            io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed").into(),
        );
    }
    if greeting.as_ref() != kokoda_protocol::VERSION.as_bytes() {
        return Err(io::Error::other("Unsupported server protocol").into());
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tcp_retries_are_bounded_and_only_for_transient_errors() {
        let mut attempts = 0;
        let mut pauses = Vec::new();
        let result = retry_connect(
            || {
                attempts += 1;
                if attempts < 3 {
                    Err(io::Error::from(io::ErrorKind::HostUnreachable))
                } else {
                    Ok(42)
                }
            },
            |delay| pauses.push(delay),
        );
        assert_eq!(result.unwrap(), 42);
        assert_eq!(
            pauses,
            vec![Duration::from_millis(500), Duration::from_secs(1)]
        );
        for (kind, expected) in [
            (io::ErrorKind::TimedOut, 3),
            (io::ErrorKind::PermissionDenied, 1),
        ] {
            let mut count = 0;
            let result: io::Result<()> = retry_connect(
                || {
                    count += 1;
                    Err(kind.into())
                },
                |_| {},
            );
            assert_eq!(result.unwrap_err().kind(), kind);
            assert_eq!(count, expected);
        }
    }

    use std::{
        net::TcpListener,
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn blocked_tls_video_write_still_receives_control_input() {
        use std::io::{Read, Write};
        struct BlockedWrite<'a>(&'a mut TcpStream);
        impl Read for BlockedWrite<'_> {
            fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
                self.0.read(b)
            }
        }
        impl Write for BlockedWrite<'_> {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::WouldBlock.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let directory = std::env::temp_dir().join(format!(
            "kokoda-duplex-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        initialize(&directory).unwrap();
        let config = server_config(&directory).unwrap();
        let token = pairing_token(&directory).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let mut authenticated = accept(listener.accept().unwrap().0, config, &token).unwrap();
            let stream = authenticated.stream_mut();
            stream.sock.set_nonblocking(true).unwrap();
            let mut blocked = BlockedWrite(&mut stream.sock);
            let mut c = Connection::new(rustls::Stream::new(&mut stream.conn, &mut blocked));
            c.queue_send(&vec![7; 64 * 1024]).unwrap();
            c.flush_pending().unwrap();
            assert!(c.has_pending_write());
            tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                c.flush_pending().unwrap();
                match c.receive() {
                    Ok(data) => {
                        assert_eq!(&data[..], b"release");
                        assert!(c.has_pending_write());
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("Control read failed: {e}"),
                }
            }
        });
        let mut client = connect(&address, &directory).unwrap();
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        client.send(b"release").unwrap();
        server.join().unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn paired_connection_rejects_wrong_token_and_wrong_certificate() {
        let directory = std::env::temp_dir().join(format!(
            "kokoda-tls-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        initialize(&directory).unwrap();
        let config = server_config(&directory).unwrap();
        let token = pairing_token(&directory).unwrap();
        let original_certificate = fs::read(directory.join("certificate.der")).unwrap();
        for scenario in 0..3 {
            fs::write(
                directory.join("pairing-token"),
                if scenario == 1 { [0; 32] } else { token },
            )
            .unwrap();
            if scenario == 2 {
                let other = generate_simple_self_signed(vec!["kokoda.local".into()]).unwrap();
                fs::write(directory.join("certificate.der"), other.cert.der()).unwrap();
            } else {
                fs::write(directory.join("certificate.der"), &original_certificate).unwrap();
            }
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let config = config.clone();
            let server = thread::spawn(move || -> Result<()> {
                let (socket, _) = listener.accept()?;
                let mut connection = accept(socket, config, &token)?;
                let request = connection.receive()?;
                connection.send(&request)?;
                Ok(())
            });
            let result = connect(&address.to_string(), &directory).and_then(|mut connection| {
                connection.send(b"authenticated window request")?;
                let received = connection.receive()?;
                if received.as_ref() != b"authenticated window request" {
                    return Err("Round trip changed payload".into());
                }
                Ok(())
            });
            let server_result = server.join().unwrap();
            assert_eq!(result.is_ok(), scenario == 0);
            assert_eq!(server_result.is_ok(), scenario == 0);
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
