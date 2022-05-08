use crate::data::CLIENTS;

use std::{net::IpAddr, sync::Arc};

use color_eyre::eyre::{bail, eyre, Result};
use futures::{StreamExt, TryFutureExt};
use rustls::{Certificate, PrivateKey};
use tracing::{error, info, info_span, Instrument};

#[allow(unused)]
pub const ALPN_QUIC_TANK_WARS: &[&[u8]] = &[b"tank-wars-prot", b"hq-29"];
pub const EXPECTED_MTU: usize = 1350;

pub struct Server {
    port: u16,
    key_log: bool,
}

impl Server {
    pub fn new(port: u16, key_log: bool) -> Self {
        Self { port, key_log }
    }

    pub async fn start(&mut self) -> Result<()> {
        let (certs, key) = Self::get_certs().await?;

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        server_crypto.alpn_protocols = ALPN_QUIC_TANK_WARS.iter().map(|&x| x.into()).collect();
        if self.key_log {
            server_crypto.key_log = Arc::new(rustls::KeyLogFile::new());
        }

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_crypto));
        server_config.use_retry(true);

        let (endpoint, mut incoming) =
            quinn::Endpoint::server(server_config, format!("0.0.0.0:{}", self.port).parse()?)?;
        self.port = endpoint.local_addr()?.port();
        info!("listening on {}", endpoint.local_addr()?);

        while let Some(conn) = incoming.next().await {
            info!("connection incoming");
            let fut = Self::handle_connection(conn);
            tokio::spawn(async move {
                if let Err(e) = fut.await {
                    error!("connection failed: {reason}", reason = e.to_string())
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(conn: quinn::Connecting) -> Result<()> {
        let mut conn = conn.await?;
        let span = info_span!(
            "connection",
            remote = %conn.connection.remote_address(),
            protocol = %conn.connection
                .handshake_data()
                .unwrap()
                .downcast::<quinn::crypto::rustls::HandshakeData>().unwrap()
                .protocol
                .map_or_else(|| "<none>".into(), |x| String::from_utf8_lossy(&x).into_owned())
        );

        async {
            info!("established");

            assert!(CLIENTS.insert(conn.connection.stable_id(), 0).is_none());

            // Each stream initiated by the client constitutes a new request.
            while let Some(stream) = conn.bi_streams.next().await {
                let stream = match stream {
                    Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                        info!("connection closed by peer");
                        assert!(CLIENTS.remove(&conn.connection.stable_id()).is_some());
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(s) => s,
                };

                let fut = Self::handle_request(stream);
                tokio::spawn(
                    async move {
                        if let Err(e) = fut.await {
                            error!("failed: {reason}", reason = e.to_string());
                        }
                    }
                    .instrument(info_span!("bidi_request")),
                );
            }
            Ok(())
        }
        .instrument(span)
        .await?;

        Ok(())
    }

    async fn handle_request(
        (mut send, mut recv): (quinn::SendStream, quinn::RecvStream),
    ) -> Result<()> {
        let mut buf = vec![0u8; EXPECTED_MTU];
        //let mut serializer = rmp_serde::Serializer::new(&mut buf);
        while let Ok(Some(len)) = recv.read(&mut buf).await {
            if len != 0 {
                println!("{:?}", len);
            }
        }

        Ok(())
    }

    #[inline(always)]
    async fn get_certs() -> Result<(Vec<Certificate>, PrivateKey)> {
        if let Some(dirs) = directories_next::ProjectDirs::from("org", "tank-wars", "tank wars") {
            let path = dirs.data_local_dir();

            let cert_path = path.join("cert.der");
            println!("{:?}", cert_path);

            let key_path = path.join("key.der");

            let (cert, key) = match tokio::fs::read(&cert_path)
                .and_then(|x| async { Ok((x, tokio::fs::read(&key_path).await?)) })
                .await
            {
                Ok(x) => x,
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    info!("generating self-signed certificate");
                    let cert =
                        rcgen::generate_simple_self_signed(vec!["localhost".into(), "tank_wars".into()]).unwrap();
                    let key = cert.serialize_private_key_der();
                    let cert = cert.serialize_der().unwrap();
                    tokio::fs::create_dir_all(&path).await?;
                    tokio::fs::write(&cert_path, &cert).await?;
                    tokio::fs::write(&key_path, &key).await?;
                    (cert, key)
                }
                Err(e) => {
                    bail!("failed to read certificate: {}", e);
                }
            };

            let key = rustls::PrivateKey(key);
            let cert = rustls::Certificate(cert);
            return Ok((vec![cert], key));
        }
        Err(eyre!("unable to get project dirs"))
    }
}
