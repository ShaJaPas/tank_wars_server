use crate::{
    data::{self, Client, Player, CLIENTS},
    db,
};

use std::sync::Arc;

use color_eyre::eyre::{bail, eyre, Result};
use futures::{StreamExt, TryFutureExt};
use rmp_serde::{Deserializer, Serializer};
use rustls::{Certificate, PrivateKey};
use serde::{Deserialize, Serialize};
use tracing::{error, info, info_span, warn, Instrument};

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
        CLIENTS.set(dashmap::DashMap::new());
        db::ID_GEN.set({
            let gen = snowflake::SnowflakeIdGenerator::new(1, 1);
            tokio::sync::Mutex::new(gen)
        });

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
            tokio::spawn(async move {
                if let Ok(conn) = conn.await {
                    let id = conn.connection.stable_id();
                    let fut = Self::handle_connection(conn);
                    tokio::spawn(async move {
                        if let Err(e) = fut.await {
                            assert!(CLIENTS.get().remove(&id).is_some());
                            error!("connection failed: {reason}", reason = e.to_string());
                        }
                    });
                } else {
                    error!("failed to open NewConnection");
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(mut conn: quinn::NewConnection) -> Result<()> {
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

            assert!(CLIENTS
                .get()
                .insert(conn.connection.stable_id(), Client::default())
                .is_none());

            // Each stream initiated by the client constitutes a new request.
            while let Some(stream) = conn.bi_streams.next().await {
                let stream = match stream {
                    Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                        info!("connection closed by peer");
                        assert!(CLIENTS.get().remove(&conn.connection.stable_id()).is_some());
                        return Ok(());
                    }
                    Err(e) => {
                        return Err(e);
                    }
                    Ok(s) => s,
                };

                let fut = Self::handle_request(stream, conn.connection.stable_id());
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
        (mut send, recv): (quinn::SendStream, quinn::RecvStream),
        conn_id: usize,
    ) -> Result<()> {
        let mut buf = Vec::with_capacity(EXPECTED_MTU);
        let mut serializer = Serializer::new(&mut buf);
        match send.id().index() {
            data::LOGIN_STREAM_ID => {
                let data = recv.read_to_end(EXPECTED_MTU).await?;
                let packet = data::Packet::deserialize(&mut Deserializer::new(data.as_slice()))?;
                if let data::Packet::SignInRequest { os_id, client_id } = packet {
                    if client_id.is_none() {
                        let id = db::ID_GEN.get().lock().await.real_time_generate();
                        let packet = data::Packet::SignInResponse {
                            client_id: Some(id),
                        };
                        let player = Player::new(id, os_id);
                        db::save(player)?;
                        CLIENTS.get().get_mut(&conn_id).unwrap().id = id;
                        info!("client sign up");
                        packet.serialize(&mut serializer)?;
                        send.write_all(&buf).await?;
                    } else {
                        if db::os_id_matches(client_id.unwrap(), os_id)? {
                            let packet = data::Packet::SignInResponse { client_id };
                            CLIENTS.get().get_mut(&conn_id).unwrap().id = client_id.unwrap();
                            info!("client sign in");
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        } else {
                            let packet = data::Packet::SignInResponse { client_id: None };
                            warn!("client sign in error");
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        }
                    }
                } else {
                    error!("Wrong data came from {} stream!", send.id().index());
                }
            }
            data::DATA_SYNC_STREAM_ID => {
                let data = recv.read_to_end(usize::MAX).await?;
                let packet = data::Packet::deserialize(&mut Deserializer::new(data.as_slice()))?;
                if let data::Packet::FilesSyncRequest { file_names } = packet {
                    let mut files = Vec::new();
                    if let Ok(mut dir) = tokio::fs::read_dir("Tanks").await {
                        while let Ok(Some(entry)) = dir.next_entry().await {
                            if entry.path().is_file()
                                && entry.path().extension().map_or(false, |v| v == "json")
                            {
                                let path = entry.path();
                                let path = path.as_os_str().to_str().unwrap();
                                let mut value = file_names
                                    .get(path)
                                    .map_or(Vec::<u8>::new(), |v| v.to_vec());

                                let signature = if value.len() != 0 {
                                    fast_rsync::Signature::deserialize(&value)?
                                } else {
                                    fast_rsync::Signature::calculate(
                                        &Vec::new(),
                                        &mut value,
                                        fast_rsync::SignatureOptions {
                                            block_size: 64,
                                            crypto_hash_size: 5,
                                        },
                                    )
                                };

                                let content = tokio::fs::read(path).await?;
                                let mut patch = Vec::new();
                                fast_rsync::diff(&signature.index(), &content, &mut patch)?;
                                files.push((path.to_owned(), patch));
                            }
                        }
                    }
                    let packet = data::Packet::FilesSyncResponse { file_names: files };
                    packet.serialize(&mut serializer)?;
                    send.write_all(&buf).await?;
                } else {
                    error!("Wrong data came from {} stream!", send.id().index());
                }
            }
            _ => {
                todo!()
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
                    let cert = rcgen::generate_simple_self_signed(vec![
                        "localhost".into(),
                        "tank_wars".into(),
                    ])
                    .unwrap();
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
