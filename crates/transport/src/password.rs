//! Password-authenticated TLS. No application data is accepted before OPAQUE
//! authenticates the TLS exporter. The ephemeral certificate alone is NOT trust.
use crate::{
    connection::Connection,
    tls::{Client, Result, Server},
};
use opaque_ke::{
    argon2::Argon2, CipherSuite, ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, CredentialFinalization, CredentialRequest,
    CredentialResponse, ServerLogin, ServerLoginParameters, ServerRegistration, ServerSetup,
};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    ClientConfig, ClientConnection, DigitallySignedStruct, ServerConfig, ServerConnection,
    SignatureScheme, StreamOwned,
};
use std::{
    collections::VecDeque,
    io::{self, BufRead, Read, Write},
    net::{IpAddr, TcpStream, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const VERSION: &[u8] = b"kokoda-password-1";
const ACCOUNT: &[u8] = b"kokoda-host-owner";
const RECORD: &[u8] = b"KOKODA-OPAQUE-1\0";
const MAX_AUTH: usize = 4096;
struct Suite;
impl CipherSuite for Suite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::TripleDh<opaque_ke::Ristretto255, sha2::Sha512>;
    type Ksf = Argon2<'static>;
}
fn denied() -> Box<dyn std::error::Error + Send + Sync> {
    io::Error::new(io::ErrorKind::PermissionDenied, "Authentication failed").into()
}
fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}
pub fn validate_password(password: &str) -> bool {
    (8..=128).contains(&password.chars().count())
        && !password.chars().any(char::is_control)
        && !password.trim().is_empty()
}
/// Read a single bounded credential prelude from a local pipe, never argv/env.
pub fn read_secret(reader: &mut impl std::io::BufRead) -> Result<Zeroizing<String>> {
    let mut bytes = Zeroizing::new(Vec::new());
    reader.take(2049).read_until(b'\n', &mut bytes)?;
    if bytes.len() > 2048 || bytes.last() != Some(&b'\n') {
        return Err(denied());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    let secret = match value.get_mut("password").map(serde_json::Value::take) {
        Some(serde_json::Value::String(s)) => Zeroizing::new(s),
        _ => return Err(denied()),
    };
    if !validate_password(&secret) {
        return Err(denied());
    }
    Ok(secret)
}
/// Local enrollment only. This record contains server secrets, not a plaintext
/// password; the platform must protect it at rest (Windows DPAPI current user).
pub fn enroll(password: &str) -> Result<Zeroizing<Vec<u8>>> {
    if !validate_password(password) {
        return Err("Password must contain 8–128 characters without control characters".into());
    }
    let mut rng = UnwrapErr(SysRng);
    let setup = ServerSetup::<Suite>::new(&mut rng);
    let client = ClientRegistration::<Suite>::start(&mut rng, password.as_bytes())?;
    let server = ServerRegistration::<Suite>::start(&setup, client.message, ACCOUNT)?;
    let finished = client.state.finish(
        &mut rng,
        password.as_bytes(),
        server.message,
        ClientRegistrationFinishParameters::default(),
    )?;
    let registration = ServerRegistration::<Suite>::finish(finished.message);
    let setup_bytes = Zeroizing::new(setup.serialize().to_vec());
    let registration_bytes = Zeroizing::new(registration.serialize().to_vec());
    let mut result = Zeroizing::new(RECORD.to_vec());
    result.extend_from_slice(&(setup_bytes.len() as u16).to_be_bytes());
    result.extend_from_slice(&setup_bytes);
    result.extend_from_slice(&registration_bytes);
    Ok(result)
}
pub struct Credentials {
    setup: ServerSetup<Suite>,
    registration: Zeroizing<Vec<u8>>,
}
impl Credentials {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_AUTH || !bytes.starts_with(RECORD) || bytes.len() < RECORD.len() + 2 {
            return Err("Invalid password record".into());
        }
        let p = RECORD.len();
        let len = u16::from_be_bytes([bytes[p], bytes[p + 1]]) as usize;
        let start = p + 2;
        let end = start + len;
        if end >= bytes.len() {
            return Err("Invalid password record".into());
        }
        let setup = ServerSetup::<Suite>::deserialize(&bytes[start..end])?;
        ServerRegistration::<Suite>::deserialize(&bytes[end..])?;
        Ok(Self {
            setup,
            registration: Zeroizing::new(bytes[end..].to_vec()),
        })
    }
}
fn send(stream: &mut impl Write, value: &[u8]) -> Result<()> {
    if value.len() > MAX_AUTH {
        return Err(denied());
    }
    stream.write_all(&(value.len() as u16).to_be_bytes())?;
    stream.write_all(value)?;
    stream.flush()?;
    Ok(())
}
fn receive(stream: &mut impl Read) -> Result<Vec<u8>> {
    let mut header = [0; 2];
    stream.read_exact(&mut header)?;
    let len = u16::from_be_bytes(header) as usize;
    if len == 0 || len > MAX_AUTH {
        return Err(denied());
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}
/// Trust is deferred to OPAQUE, but TLS signature verification still proves
/// possession of this connection's certificate key. Never use without OPAQUE.
#[derive(Debug)]
struct PendingPasswordIdentity;
impl ServerCertVerifier for PendingPasswordIdentity {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General(
            "Password connections require TLS 1.3".into(),
        ))
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            signature,
            &provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
fn ephemeral_config() -> Result<Arc<ServerConfig>> {
    let identity = rcgen::generate_simple_self_signed(vec!["kokoda.local".into()])?;
    let key = PrivatePkcs8KeyDer::from(identity.key_pair.serialize_der());
    let mut config = ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![identity.cert.der().clone()], key.into())?;
    config.max_early_data_size = 0;
    config.send_tls13_tickets = 0;
    Ok(Arc::new(config))
}
fn socket_options(socket: &TcpStream) -> Result<()> {
    socket.set_nodelay(true)?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    Ok(())
}
fn context(binding: [u8; 32]) -> Vec<u8> {
    [VERSION, &binding].concat()
}
fn server_exchange(
    stream: &mut (impl Read + Write),
    credentials: &Credentials,
    binding: [u8; 32],
) -> Result<()> {
    if receive(stream)?.as_slice() != VERSION {
        return Err(denied());
    }
    let request =
        CredentialRequest::<Suite>::deserialize(&receive(stream)?).map_err(|_| denied())?;
    let mut rng = UnwrapErr(SysRng);
    let context = context(binding);
    let params = ServerLoginParameters {
        context: Some(&context),
        ..Default::default()
    };
    let login = ServerLogin::start(
        &mut rng,
        &credentials.setup,
        Some(ServerRegistration::deserialize(&credentials.registration)?),
        request,
        ACCOUNT,
        params.clone(),
    )
    .map_err(|_| denied())?;
    send(stream, &login.message.serialize())?;
    let finalization =
        CredentialFinalization::deserialize(&receive(stream)?).map_err(|_| denied())?;
    let mut finished = login
        .state
        .finish(finalization, params)
        .map_err(|_| denied())?;
    finished.session_key.zeroize();
    send(stream, VERSION)?;
    Ok(())
}
fn client_exchange(
    stream: &mut (impl Read + Write),
    password: &str,
    binding: [u8; 32],
) -> Result<()> {
    let mut rng = UnwrapErr(SysRng);
    let login = ClientLogin::<Suite>::start(&mut rng, password.as_bytes()).map_err(|_| denied())?;
    send(stream, VERSION)?;
    send(stream, &login.message.serialize())?;
    let response = CredentialResponse::deserialize(&receive(stream)?).map_err(|_| denied())?;
    let context = context(binding);
    let mut finished = login
        .state
        .finish(
            &mut rng,
            password.as_bytes(),
            response,
            ClientLoginFinishParameters {
                context: Some(&context),
                ..Default::default()
            },
        )
        .map_err(|_| denied())?;
    send(stream, &finished.message.serialize())?;
    finished.session_key.zeroize();
    if receive(stream)?.as_slice() != VERSION {
        return Err(denied());
    }
    Ok(())
}
pub fn accept(socket: TcpStream, credentials: &Credentials) -> Result<Server> {
    socket_options(&socket)?;
    // Fresh certificate and ephemeral TLS traffic keys for every connection.
    let mut stream = StreamOwned::new(ServerConnection::new(ephemeral_config()?)?, socket);
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock)?;
    }
    let binding =
        stream
            .conn
            .export_keying_material([0; 32], b"EXPORTER-Kokoda-OPAQUE-v1", Some(VERSION))?;
    server_exchange(&mut stream, credentials, binding)?;
    Ok(Connection::new(stream))
}
pub fn connect(address: &str, password: &str) -> Result<Client> {
    if !validate_password(password) {
        return Err(denied());
    }
    let config = ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PendingPasswordIdentity))
        .with_no_client_auth();
    let endpoint = address
        .to_socket_addrs()?
        .next()
        .ok_or("No server address")?;
    let socket = crate::tls::retry_connect(
        || TcpStream::connect_timeout(&endpoint, Duration::from_secs(5)),
        std::thread::sleep,
    )?;
    socket_options(&socket)?;
    let mut stream = StreamOwned::new(
        ClientConnection::new(Arc::new(config), ServerName::try_from("kokoda.local")?)?,
        socket,
    );
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock)?;
    }
    let binding =
        stream
            .conn
            .export_keying_material([0; 32], b"EXPORTER-Kokoda-OPAQUE-v1", Some(VERSION))?;
    client_exchange(&mut stream, password, binding)?;
    Ok(Connection::new(stream))
}
/// Fixed memory, global + peer attempt budget. Success does not reset it, so a
/// password holder cannot accelerate guessing another owner's new password.
#[derive(Default)]
pub struct AttemptLimiter {
    attempts: VecDeque<(Instant, IpAddr)>,
}
impl AttemptLimiter {
    pub fn allow(&mut self, now: Instant, peer: IpAddr) -> bool {
        while self
            .attempts
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(60))
        {
            self.attempts.pop_front();
        }
        if self.attempts.len() >= 20
            || self.attempts.iter().filter(|(_, ip)| *ip == peer).count() >= 5
        {
            return false;
        }
        self.attempts.push_back((now, peer));
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};
    const PASSWORD: &str = "Synthetic password 例子";
    #[test]
    fn password_roundtrip_wrong_password_and_certificate_rotation() {
        let record = enroll(PASSWORD).unwrap();
        assert!(!record
            .windows(PASSWORD.len())
            .any(|w| w == PASSWORD.as_bytes()));
        let mut certificates = Vec::new();
        for password in [PASSWORD, "Wrong synthetic password", PASSWORD] {
            let credentials = Credentials::decode(&record).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let server = thread::spawn(move || -> Result<()> {
                let mut connection = accept(listener.accept()?.0, &credentials)?;
                let bytes = connection.receive()?;
                connection.send(&bytes)?;
                Ok(())
            });
            let result = connect(&address, password).and_then(|mut c| {
                certificates.push(c.stream_mut().conn.peer_certificates().unwrap()[0].to_vec());
                c.send(b"authorized")?;
                assert_eq!(c.receive()?.as_ref(), b"authorized");
                Ok(())
            });
            assert_eq!(result.is_ok(), password == PASSWORD);
            assert_eq!(server.join().unwrap().is_ok(), password == PASSWORD);
        }
        assert_ne!(certificates[0], certificates[1]);
    }
    #[test]
    fn channel_binding_blocks_relay_and_password_rotation_rejects_old_secret() {
        for (password, binding) in [(PASSWORD, [2; 32]), ("Old synthetic password", [1; 32])] {
            let credentials = Credentials::decode(&enroll(PASSWORD).unwrap()).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let mut s = listener.accept().unwrap().0;
                socket_options(&s).unwrap();
                server_exchange(&mut s, &credentials, [1; 32])
            });
            let mut socket = TcpStream::connect(address).unwrap();
            socket_options(&socket).unwrap();
            assert!(client_exchange(&mut socket, password, binding).is_err());
            drop(socket);
            assert!(server.join().unwrap().is_err());
        }
    }
    #[test]
    fn local_password_prelude_is_bounded_and_does_not_consume_commands() {
        let mut reader = std::io::Cursor::new(
            b"{\"password\":\"Synthetic password\"}\n{\"command\":\"ping\"}\n",
        );
        assert_eq!(&*read_secret(&mut reader).unwrap(), "Synthetic password");
        let mut next = String::new();
        reader.read_line(&mut next).unwrap();
        assert!(next.contains("ping"));
        for bytes in [
            b"{bad}\n".as_slice(),
            b"{\"password\":3}\n",
            b"{\"password\":\"short\"}\n",
        ] {
            assert_eq!(
                read_secret(&mut std::io::Cursor::new(bytes))
                    .unwrap_err()
                    .to_string(),
                "Authentication failed"
            );
        }
        assert!(read_secret(&mut std::io::Cursor::new(vec![b'x'; 4096])).is_err());
    }
    #[test]
    fn auth_frames_and_attempts_are_bounded() {
        assert!(receive(&mut &b"\xff\xff"[..]).is_err());
        assert!(Credentials::decode(b"invalid").is_err());
        assert!(!validate_password("short"));
        assert!(!validate_password("password\n123"));
        let now = Instant::now();
        let ip = "127.0.0.1".parse().unwrap();
        let mut limiter = AttemptLimiter::default();
        for _ in 0..5 {
            assert!(limiter.allow(now, ip));
        }
        assert!(!limiter.allow(now, ip));
        assert!(limiter.allow(now + Duration::from_secs(60), ip));
        for peer in 1..20 {
            assert!(limiter.allow(
                now + Duration::from_secs(60),
                IpAddr::from([10, 0, 0, peer])
            ));
        }
        assert!(!limiter.allow(now + Duration::from_secs(60), IpAddr::from([10, 0, 0, 100])));
    }
}
