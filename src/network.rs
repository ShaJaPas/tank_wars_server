use crate::{
    data::{
        self, BalancerCommand, Chest, ChestName, Client, Player, PlayerPosition, CLIENTS,
        MATCHMAKER, NICKNAME_REGEX, PHYSICS,
    },
    db,
    physics::{self, BalancedPlayer, PhysicsCommand},
};

use std::{io::Cursor, str::FromStr, sync::Arc};

use color_eyre::eyre::{bail, eyre, Result};
use futures::{StreamExt, TryFutureExt};
use minstant::Instant;
use rmp_serde::{Deserializer, Serializer};
use rustls::{Certificate, PrivateKey};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, info_span, warn, Instrument};

#[allow(unused)]
pub const ALPN_QUIC_TANK_WARS: &[&[u8]] = &[b"tank-wars-prot"];
pub const EXPECTED_MTU: usize = 1350;
pub const TWELVE_HOURS: i64 = 12 * 60 * 60;
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
        NICKNAME_REGEX.set(regex::Regex::new(r"[a-zA-Z]\w{5,14}").unwrap());
        db::ID_GEN.set({
            let gen = snowflake::SnowflakeIdGenerator::new(1, 1);
            parking_lot::Mutex::new(gen)
        });

        //Balancer initialization
        let (send, recv) = flume::unbounded::<BalancerCommand>();
        let p_send = physics::start();
        MATCHMAKER.set(move || send.clone());
        PHYSICS.set(move || p_send.clone());
        tokio::spawn(async move {
            let fut = Self::balance_players(recv);
            if let Err(e) = fut.await {
                error!("{}", e);
            }
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
                            let client = CLIENTS.get().remove(&id).unwrap();
                            MATCHMAKER
                                .get()
                                .send(BalancerCommand::RemovePlayer(client.1.id))
                                .unwrap();
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

    //Simple balancer, gets two players with diff <= 60.
    //Prefers player with lowest number
    //Number only increments, so it is limited to i32::MAX_VALUE
    //TODO: improve
    async fn balance_players(recv: flume::Receiver<BalancerCommand>) -> Result<()> {
        const DIFF: u32 = 60;
        let mut number = 0;
        let mut list: Vec<(BalancedPlayer, i32)> = Vec::new();
        while let Ok(x) = recv.recv_async().await {
            match x {
                BalancerCommand::AddPlayer {
                    player,
                    tank_id,
                    conn,
                } => {
                    if list.iter().any(|f| f.0 .0.id == player.id) {
                        continue;
                    }
                    list.push((BalancedPlayer(player, tank_id, conn), number));
                    number += 1;
                    list.sort_by(|a, b| a.0 .0.trophies.cmp(&b.0 .0.trophies));
                    for i in 0..list.len() {
                        let mut best_match: Option<&(BalancedPlayer, i32)> = None;
                        for j in (i + 1)..list.len() {
                            if list[j].0 .0.trophies.abs_diff(list[i].0 .0.trophies) <= DIFF {
                                if best_match.is_none() || list[j].1 < best_match.unwrap().1 {
                                    best_match = Some(&list[j]);
                                }
                            } else {
                                break;
                            }
                        }
                        if best_match.is_some() {
                            let index = list
                                .iter()
                                .position(|f| f.0 .0.id == list[i].0 .0.id)
                                .unwrap();
                            let id = best_match.unwrap().0 .0.id;
                            let player1 = list.remove(index).0;
                            let index = list.iter().position(|f| f.0 .0.id == id).unwrap();
                            let player2 = list.remove(index).0;
                            PHYSICS
                                .get()
                                .send(physics::PhysicsCommand::CreateMatch {
                                    players: (player1, player2),
                                })
                                .unwrap();
                        }
                    }
                }
                BalancerCommand::RemovePlayer(id) => {
                    if let Some(index) = list.iter().position(|f| f.0 .0.id == id) {
                        list.remove(index);
                    }
                }
            }
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
                .insert(conn.connection.stable_id(), Client {
                    id: 0
                })
                .is_none());

            loop{
                tokio::select! {
                    biased;

                    Some(stream) = conn.bi_streams.next() => {
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

                        let fut = Self::handle_request(stream, conn.connection.clone());
                        tokio::spawn(
                            async move {
                                let instant = Instant::now();
                                match fut.await {
                                    Err(e) => error!("failed: {reason}", reason = e.to_string()),
                                    Ok(name) => debug!("Request {name} handled in {:?}", instant.elapsed()),
                                }
                            }
                            .instrument(info_span!("bidi_request")),
                        );
                    },

                    Some(stream) = conn.uni_streams.next() => {
                        let stream = match stream {
                            Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                                info!("connection closed by peer");
                                let client = CLIENTS.get().remove(&conn.connection.stable_id()).unwrap();
                                MATCHMAKER.get().send(BalancerCommand::RemovePlayer(client.1.id)).unwrap();
                                return Ok(());
                            }
                            Err(e) => {
                                return Err(e);
                            }
                            Ok(s) => s,
                        };

                        let fut = Self::handle_uni_stream(stream, conn.connection.clone());
                        tokio::spawn(
                            async move {
                                let instant = Instant::now();
                                match fut.await {
                                    Err(e) => error!("failed: {reason}", reason = e.to_string()),
                                    Ok(name) => {
                                        let elapsed = instant.elapsed();
                                        debug!("Request {name} handled in {:?}", elapsed);
                                    }
                                }
                            }
                            .instrument(info_span!("bidi_request")),
                        );
                    },

                    Some(stream) = conn.datagrams.next() => {
                        let buf = match stream {
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

                        if let Ok(packet) = PlayerPosition::deserialize(&mut Deserializer::new(buf.as_ref())){
                            if let Some(client) = CLIENTS.get().get(&conn.connection.stable_id()){
                                let cmd = PhysicsCommand::PlayerPacket {id: client.id, position: packet};
                                PHYSICS.get().send(cmd).unwrap();
                            }
                        }

                    },

                    else => break,
                }
            }

            Ok(())
        }
        .instrument(span)
        .await?;

        Ok(())
    }

    async fn handle_uni_stream(recv: quinn::RecvStream, conn: quinn::Connection) -> Result<String> {
        let data = recv.read_to_end(usize::MAX).await.unwrap();
        let packet = data::Packet::deserialize(&mut Deserializer::new(data.as_slice()))?;
        let enum_name = packet.to_string();
        match packet {
            data::Packet::GetChestRequest { name } => match name {
                ChestName::COMMON => {
                    let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                    if id.is_none() {
                        error!("unauthorized access");
                        return Ok(enum_name);
                    }
                    let mut buf = Vec::new();
                    let mut serializer = Serializer::new(&mut buf);
                    let mut player = db::get_player_by_id(id.unwrap()).unwrap();
                    if player.coins >= ChestName::COMMON as i32 {
                        player.coins -= ChestName::COMMON as i32;
                        let chest = Chest::generate_random_loot(ChestName::COMMON, &player);
                        chest.add_to_player(&mut player);
                        let packet = data::Packet::GetChestResponse { chest };
                        packet.serialize(&mut serializer)?;
                        let mut send = conn.open_uni().await?;
                        send.write_all(&buf).await?;
                        send.finish().await?;
                        player.check_daily_items();
                        db::update_player(&player)?;
                    }
                }
                _ => unimplemented!(),
            },
            data::Packet::JoinMatchMakerRequest { id: tank_id } => {
                let client = CLIENTS.get().get(&conn.stable_id());
                let id = client.as_ref().map(|f| f.id);
                let conn = conn.clone();
                if id.is_none() {
                    error!("unauthorized access");
                    return Ok(enum_name);
                }
                let player = db::get_player_by_id(id.unwrap()).unwrap();
                MATCHMAKER
                    .get()
                    .send(BalancerCommand::AddPlayer {
                        player: Box::new(player),
                        tank_id,
                        conn,
                    })
                    .unwrap();
            }
            data::Packet::LeaveMatchMakerRequest => {
                let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                if id.is_none() {
                    error!("unauthorized access");
                    return Ok(enum_name);
                }
                MATCHMAKER
                    .get()
                    .send(BalancerCommand::RemovePlayer(id.unwrap()))
                    .unwrap();
            }
            data::Packet::Shoot => {
                if let Some(client) = CLIENTS.get().get(&conn.stable_id()) {
                    let cmd = PhysicsCommand::PlayerShoot { id: client.id };
                    PHYSICS.get().send(cmd).unwrap();
                }
            }
            _ => {
                error!("wrong packet came from uni stream!");
            }
        }
        Ok(enum_name)
    }

    async fn handle_request(
        (mut send, mut recv): (quinn::SendStream, quinn::RecvStream),
        conn: quinn::Connection,
    ) -> Result<String> {
        let mut data = Vec::with_capacity(EXPECTED_MTU);
        let mut enum_name = String::new();
        while let Some(chunk) = recv.read_chunk(usize::MAX, true).await? {
            data.extend(chunk.bytes);
            let mut de = Deserializer::new(Cursor::new(&data));
            if let Ok(packet) = data::Packet::deserialize(&mut de) {
                let size: usize = de.position() as usize;
                enum_name = packet.to_string();
                data.drain(0..size);
                match packet {
                    data::Packet::SignInRequest { os_id, client_id } => {
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        if client_id.is_none() {
                            let id = db::ID_GEN.get().lock().real_time_generate();
                            let player = Player::new(id, os_id);
                            db::save(&player)?;
                            CLIENTS.get().get_mut(&conn.stable_id()).unwrap().id = id;
                            info!("client sign up");
                            let packet = data::Packet::SignInResponse {
                                client_id: Some(id),
                                profile: Some(player),
                            };
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        } else if db::os_id_matches(client_id.unwrap(), os_id)? {
                            CLIENTS.get().get_mut(&conn.stable_id()).unwrap().id =
                                client_id.unwrap();
                            info!("client sign in");
                            let mut player = db::get_player_by_id(client_id.unwrap()).unwrap();

                            //check daily items
                            let time = chrono::Utc::now().naive_utc();
                            if (time - player.daily_items_time).num_seconds() >= TWELVE_HOURS {
                                player.daily_items_time = time;
                                player.daily_items = player.get_daily_items();
                                db::update_player(&player)?;
                            }

                            let packet = data::Packet::SignInResponse {
                                client_id,
                                profile: Some(player),
                            };
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        } else {
                            let packet = data::Packet::SignInResponse {
                                client_id: None,
                                profile: None,
                            };
                            warn!("client sign in error");
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        }
                    }
                    data::Packet::FilesSyncRequest { file_names } => {
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
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

                                    let signature = if !value.is_empty() {
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

                        //Check if player already is in battle
                        if let Some(id) = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id) {
                            PHYSICS
                                .get()
                                .send(PhysicsCommand::NotifyPlayerAboutMatch {
                                    id,
                                    new_conn: conn.clone(),
                                })
                                .unwrap();
                        }
                    }
                    data::Packet::PlayerProfileRequest { nickname } => {
                        let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                        if id.is_none() {
                            error!("unauthorized access");
                            return Ok(enum_name);
                        }
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        let player = db::get_player_by_nickname(&nickname);
                        let player = player.map(|mut f| {
                            if id.unwrap() != f.id {
                                (f.coins, f.diamonds, f.daily_items, f.daily_items_time) =
                                    (0, 0, Vec::new(), data::default_naive_date_time());
                            }
                            f
                        });

                        let packet = data::Packet::PlayerProfileResponse {
                            profile: player,
                            nickname,
                        };
                        packet.serialize(&mut serializer)?;
                        send.write_all(&buf).await?;
                    }
                    data::Packet::SetNicknameRequest { nickname } => {
                        let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                        if id.is_none() {
                            error!("unauthorized access");
                            return Ok(enum_name);
                        }
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        if NICKNAME_REGEX.get().is_match(&nickname) {
                            if db::get_player_by_nickname(&nickname).is_none() {
                                let mut player = db::get_player_by_id(id.unwrap()).unwrap();
                                if player.nickname.is_none() {
                                    player.nickname = Some(nickname);
                                    let packet = data::Packet::SetNicknameResponse { error: None };
                                    packet.serialize(&mut serializer)?;
                                    send.write_all(&buf).await?;
                                    if player.tanks.is_empty() {
                                        let chest = Chest::generate_random_loot(
                                            ChestName::STARTER,
                                            &player,
                                        );
                                        chest.add_to_player(&mut player);
                                        let packet = data::Packet::GetChestResponse { chest };
                                        let mut buf = Vec::new();
                                        let mut serializer = Serializer::new(&mut buf);
                                        packet.serialize(&mut serializer)?;
                                        let mut uni = conn.open_uni().await?;
                                        uni.write_all(&buf).await?;
                                    }
                                    db::update_player(&player)?;
                                } else {
                                    let packet = data::Packet::SetNicknameResponse {
                                        error: Some(
                                            String::from_str("Nickname has been already set")
                                                .unwrap(),
                                        ),
                                    };
                                    packet.serialize(&mut serializer)?;
                                    send.write_all(&buf).await?;
                                }
                            } else {
                                let packet = data::Packet::SetNicknameResponse {
                                    error: Some(
                                        String::from_str(
                                            "Nickname is already registered by another player",
                                        )
                                        .unwrap(),
                                    ),
                                };
                                packet.serialize(&mut serializer)?;
                                send.write_all(&buf).await?;
                            }
                        } else {
                            let packet = data::Packet::SetNicknameResponse {
                                error: Some(String::from_str("Nickname must start from letter, its length must be in range from 6 to 15 and only contains English letters, digits and underscore").unwrap()) 
                            };
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        }
                    }
                    data::Packet::UpgradeTankRequest { id: tank_id } => {
                        let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                        if id.is_none() {
                            error!("unauthorized access");
                            return Ok(enum_name);
                        }
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        let mut player = db::get_player_by_id(id.unwrap()).unwrap();
                        if let Some(mut tank) = player.tanks.iter_mut().find(|f| f.id == tank_id) {
                            let bound = 2i32.pow(tank.level as u32 - 1u32) * 50;
                            if tank.count >= bound {
                                tank.count -= bound;
                                tank.level += 1;
                                let packet =
                                    data::Packet::UpgradeTankResponse { id: Some(tank_id) };
                                packet.serialize(&mut serializer)?;
                                send.write_all(&buf).await?;
                                db::update_player(&player)?;
                            } else {
                                let packet = data::Packet::UpgradeTankResponse { id: None };
                                packet.serialize(&mut serializer)?;
                                send.write_all(&buf).await?;
                            }
                        }
                    }
                    data::Packet::GetDailyItemsRequest => {
                        let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                        if id.is_none() {
                            error!("unauthorized access");
                            return Ok(enum_name);
                        }
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        let mut player = db::get_player_by_id(id.unwrap()).unwrap();
                        let time = chrono::Utc::now().naive_utc();
                        if (time - player.daily_items_time).num_seconds() >= TWELVE_HOURS {
                            player.daily_items_time = time;
                            player.daily_items = player.get_daily_items();
                            db::update_player(&player)?;
                            let packet = data::Packet::GetDailyItemsResponse {
                                items: player.daily_items,
                                time: Some(time),
                            };
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        } else {
                            let packet = data::Packet::GetDailyItemsResponse {
                                items: player.daily_items,
                                time: None,
                            };
                            packet.serialize(&mut serializer)?;
                            send.write_all(&buf).await?;
                        }
                    }
                    data::Packet::GetDailyItemRequest { id: number } => {
                        let id = CLIENTS.get().get_mut(&conn.stable_id()).map(|f| f.id);
                        if id.is_none() {
                            error!("unauthorized access");
                            return Ok(enum_name);
                        }
                        let mut buf = Vec::new();
                        let mut serializer = Serializer::new(&mut buf);
                        let mut player = db::get_player_by_id(id.unwrap()).unwrap();
                        let item = player.daily_items.get_mut(number as usize);
                        if let Some(item) = item {
                            if !item.bought && player.coins >= item.price {
                                item.bought = true;
                                player.coins -= item.price;
                                let tank = player.tanks.iter_mut().find(|f| f.id == item.tank_id);
                                if let Some(tank) = tank {
                                    tank.count += item.count;
                                } else {
                                    let tank = data::Tank {
                                        id: item.tank_id,
                                        level: 1,
                                        count: 0,
                                    };
                                    player.tanks.push(tank);
                                }
                                let response = data::Packet::GetDailyItemResponse {
                                    player: Some(player.clone()),
                                };
                                response.serialize(&mut serializer)?;
                                send.write_all(&buf).await?;
                                db::update_player(&player)?;
                            } else {
                                let response = data::Packet::GetDailyItemResponse { player: None };
                                response.serialize(&mut serializer)?;
                                send.write_all(&buf).await?;
                            }
                        }
                    }
                    _ => {
                        error!("Wrong data came from {} stream!", send.id().index());
                    }
                }
            }
        }
        Ok(enum_name)
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
