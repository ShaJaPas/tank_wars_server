use std::{collections::HashMap, str::FromStr};

use minstant::Instant;
use quinn::Connection;
use rand::Rng;
use rapier2d::{na::UnitComplex, prelude::*};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::{
    error::TryRecvError, unbounded_channel, UnboundedReceiver, UnboundedSender,
};

use crate::data::{
    BattleResult, BattleResultStruct, BulletData, GamePacket, GamePlayerData, Map, Packet, Player,
    PlayerPosition, Tank, TankInfo, TANKS,
};

type Result<T> = color_eyre::Result<T>;

const UPDATE_TIME: f32 = 1f32 / 30f32;
const WAIT_TIME: f32 = 5f32;
const MAX_BATTLE_TIME: f32 = 60f32 * 3f32;
const SCALE_TO_PHYSICS: f32 = 1f32 / 50f32;
const SCALE_TO_PIXELS: f32 = 50f32;

macro_rules! send_results {
    // macth like arm for macro
    ($x:ident, $gen:ident, $a:tt, $b:tt, $draw:expr) => {
        // macro expand to this code
        let acc = $x.players.$b.stats.succeeded_shots as f32 / $x.players.$b.stats.shots as f32;
        let eff = (acc + 0.5f32)
            * if ($x.players.$b.stats.damage_dealt as f32 / $x.players.$b.stats.damage_taken as f32)
                .is_normal()
            {
                ($x.players.$b.stats.damage_dealt as f32 / $x.players.$b.stats.damage_taken as f32)
            } else {
                1f32
            };

        let mut win_results = BattleResultStruct {
            result: BattleResult::Victory,
            trophies: 30
                + ($x.players.$a.player.get_efficiency() - $x.players.$b.player.get_efficiency())
                    as i32,
            xp: ((if eff.is_normal() { eff } else { 0f32 }) * 15f32) as i32,
            coins: $gen.gen_range(70..=100)
                + $gen.gen_range(10..=15)
                    * ($x.players.$a.player.rank_level - $x.players.$b.player.rank_level),
            damage_dealt: $x.players.$b.stats.damage_dealt,
            damage_taken: $x.players.$b.stats.damage_taken,
            accuracy: if acc.is_normal() { acc } else { 0f32 },
            efficiency: if eff.is_normal() { eff } else { 0f32 },
        };

        let acc = $x.players.$a.stats.succeeded_shots as f32 / $x.players.$a.stats.shots as f32;
        let eff = (acc + 0.5f32)
            * if ($x.players.$a.stats.damage_dealt as f32 / $x.players.$a.stats.damage_taken as f32)
                .is_normal()
            {
                ($x.players.$a.stats.damage_dealt as f32 / $x.players.$a.stats.damage_taken as f32)
            } else {
                1f32
            };

        let mut lose_results = BattleResultStruct {
            result: BattleResult::Defeat,
            trophies: -30
                - ($x.players.$a.player.get_efficiency() - $x.players.$b.player.get_efficiency())
                    as i32,
            xp: ((if eff.is_normal() { eff } else { 0f32 }) * 15f32) as i32,
            coins: $gen.gen_range(15..=20)
                + $gen.gen_range(10..=15)
                    * ($x.players.$b.player.rank_level - $x.players.$a.player.rank_level),
            damage_dealt: $x.players.$a.stats.damage_dealt,
            damage_taken: $x.players.$a.stats.damage_taken,
            accuracy: if acc.is_normal() { acc } else { 0f32 },
            efficiency: if eff.is_normal() { eff } else { 0f32 },
        };

        if $draw {
            win_results.result = BattleResult::Draw;
            win_results.xp = 0;
            win_results.coins = 0;
            win_results.trophies = 0;
            lose_results.result = BattleResult::Draw;
            lose_results.xp = 0;
            lose_results.coins = 0;
            lose_results.trophies = 0;
        }
        if !$draw {
            $x.players.$b.player.victories_count += 1;
        }
        $x.players.$b.player.trophies += win_results.trophies;
        $x.players.$b.player.xp += win_results.xp;
        $x.players.$b.player.coins += win_results.coins;
        let xp_bound = (3f32.powf($x.players.$b.player.rank_level as f32 / 10f32)
            * $x.players.$b.player.rank_level as f32
            * 50f32) as i32;
        if $x.players.$b.player.xp >= xp_bound {
            $x.players.$b.player.xp -= xp_bound;
            $x.players.$b.player.rank_level += 1;
        }
        $x.players.$b.player.accuracy = ($x.players.$b.player.accuracy
            * ($x.players.$b.player.battles_count as f32 - 1f32)
            + win_results.accuracy)
            / $x.players.$b.player.battles_count as f32;
        $x.players.$b.player.damage_dealt = ($x.players.$b.player.damage_dealt
            * $x.players.$b.player.battles_count
            + win_results.damage_dealt)
            / $x.players.$b.player.battles_count;
        $x.players.$b.player.damage_taken = ($x.players.$b.player.damage_taken
            * $x.players.$b.player.battles_count
            + win_results.damage_taken)
            / $x.players.$b.player.battles_count;

        $x.players.$a.player.trophies =
            0.max($x.players.$a.player.trophies + lose_results.trophies);
        $x.players.$a.player.xp += lose_results.xp;
        $x.players.$a.player.coins += lose_results.coins;
        let xp_bound = (3f32.powf($x.players.$a.player.rank_level as f32 / 10f32)
            * $x.players.$a.player.rank_level as f32
            * 50f32) as i32;
        if $x.players.$a.player.xp >= xp_bound {
            $x.players.$a.player.xp -= xp_bound;
            $x.players.$a.player.rank_level += 1;
        }
        $x.players.$a.player.accuracy = ($x.players.$a.player.accuracy
            * ($x.players.$a.player.battles_count as f32 - 1f32)
            + win_results.accuracy)
            / $x.players.$a.player.battles_count as f32;
        $x.players.$a.player.damage_dealt = ($x.players.$a.player.damage_dealt
            * $x.players.$a.player.battles_count
            + win_results.damage_dealt)
            / $x.players.$a.player.battles_count;
        $x.players.$a.player.damage_taken = ($x.players.$a.player.damage_taken
            * $x.players.$a.player.battles_count
            + win_results.damage_taken)
            / $x.players.$a.player.battles_count;

        let winner_conn = $x.players.$b.conn.clone();
        let win_profile = (*$x.players.$b.player).clone();
        let loser_conn = $x.players.$a.conn.clone();
        let loser_profile = (*$x.players.$a.player).clone();
        if let Some(_) = futures::executor::block_on(async move {
            let data = Packet::BattleResultResponse {
                profile: win_profile,
                result: win_results,
            };
            let mut buf = Vec::new();
            let mut serializer = Serializer::new(&mut buf);
            data.serialize(&mut serializer).unwrap();
            let mut uni = winner_conn.open_uni().await?;
            uni.write_all(&buf).await?;
            uni.finish().await?;
            let data = Packet::BattleResultResponse {
                profile: loser_profile,
                result: lose_results,
            };
            let mut buf = Vec::new();
            let mut serializer = Serializer::new(&mut buf);
            data.serialize(&mut serializer).unwrap();
            let mut uni = loser_conn.open_uni().await?;
            uni.write_all(&buf).await?;
            uni.finish().await?;
            Result::<()>::Ok(())
        })
        .err()
        {}
    };
}

#[derive(Debug)]
pub struct BalancedPlayer(pub Box<Player>, pub i32, pub Connection);

struct WorldPlayer<'a> {
    tank_info: &'a TankInfo,
    tank: Tank,
    player: Box<Player>,
    conn: Connection,
    stats: PlayerInfo,
    handle: RigidBodyHandle,
    connected: bool,
}

#[derive(Default)]
struct PlayerInfo {
    damage_dealt: i32,
    damage_taken: i32,
    shots: i32,
    succeeded_shots: i32,
    hp: i32,
    damage: i32,
    gun_rotation: f32,
    gun_angle: f32,
    cool_down: f32,
}

impl TryFrom<BalancedPlayer> for WorldPlayer<'_> {
    type Error = String;

    fn try_from(value: BalancedPlayer) -> std::result::Result<Self, Self::Error> {
        let tank = value
            .0
            .tanks
            .iter()
            .find(|f| f.id == value.1)
            .ok_or("Wrong id!")?
            .clone();
        Ok(Self {
            tank_info: TANKS
                .get()
                .iter()
                .find(|f| f.id as i32 == value.1)
                .ok_or("Wrong id!")?,
            player: value.0,
            conn: value.2,
            tank,
            stats: PlayerInfo::default(),
            handle: RigidBodyHandle::invalid(),
            connected: true,
        })
    }
}

#[derive(Debug)]
pub enum PhysicsCommand {
    CreateMatch {
        players: (BalancedPlayer, BalancedPlayer),
    },
    PlayerPacket {
        id: i64,
        position: PlayerPosition,
    },
    PlayerShoot {
        id: i64,
    },
    NotifyPlayerAboutMatch {
        id: i64,
        new_conn: Connection,
    },
}

enum ObjectConstants {
    RedBarrage = 1,
    YellowBarrage = 2,
    RustyBarrelStay = 3,
    RustyBarrelLay = 4,
    StealHedgehog = 5,
    WoodenHedgehog = 6,
    LargeBush = 7,
    SmallBush = 8,
}

#[repr(u64)]
#[derive(PartialEq, Debug, strum::FromRepr, Default, Clone, Copy)]
enum BodyType {
    Bullet,
    Tank,
    #[default]
    Other,
}

#[derive(PartialEq, Default, Debug, Clone, Copy)]
struct UserData {
    body_type: BodyType,
    id: i64,
}

impl UserData {
    fn new(body_type: BodyType, id: i64) -> Self {
        Self { body_type, id }
    }
}
impl From<u128> for UserData {
    fn from(value: u128) -> Self {
        let lo = (value & 0xffff_ffff_ffff_ffff) as u64;
        let hi = (value >> 64) as u64;
        Self {
            body_type: BodyType::from_repr(lo).unwrap_or_default(),
            id: hi as i64,
        }
    }
}

impl From<UserData> for u128 {
    fn from(value: UserData) -> Self {
        let lo = value.body_type as u64;
        let hi = value.id as u64;
        (hi as u128) << 64 | lo as u128
    }
}

struct CustomPhysicsHooks;

impl PhysicsHooks for CustomPhysicsHooks {
    fn filter_contact_pair(&self, context: &PairFilterContext) -> Option<SolverFlags> {
        let user_data1: UserData = context.bodies[context.rigid_body1.unwrap()]
            .user_data
            .into();
        let user_data2: UserData = context.bodies[context.rigid_body2.unwrap()]
            .user_data
            .into();
        if user_data1.id == user_data2.id
            && ((user_data1.body_type == BodyType::Bullet
                && user_data2.body_type == BodyType::Tank)
                || (user_data1.body_type == BodyType::Tank
                    && user_data2.body_type == BodyType::Bullet))
        {
            return None;
        }
        Some(SolverFlags::COMPUTE_IMPULSES)
    }

    fn filter_intersection_pair(&self, context: &PairFilterContext) -> bool {
        let user_data1: UserData = context.bodies[context.rigid_body1.unwrap()]
            .user_data
            .into();
        let user_data2: UserData = context.bodies[context.rigid_body2.unwrap()]
            .user_data
            .into();
        !(user_data1.id == user_data2.id
            && ((user_data1.body_type == BodyType::Bullet
                && user_data2.body_type == BodyType::Tank)
                || (user_data1.body_type == BodyType::Tank
                    && user_data2.body_type == BodyType::Bullet)))
    }
}

struct ChannelledEventCollector {
    collision_event_sender: UnboundedSender<(CollisionEvent, Point<Real>)>,
}

impl EventHandler for ChannelledEventCollector {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: CollisionEvent,
        pair: Option<&ContactPair>,
    ) {
        if let Some(pair) = pair {
            let point = pair
                .manifolds
                .iter()
                .find(|f| f.data.solver_contacts.get(0).is_some());

            if let Some(point) = point {
                let _ = self
                    .collision_event_sender
                    .send((event, point.data.solver_contacts[0].point));
            }
        }
    }

    fn handle_contact_force_event(
        &self,
        _dt: Real,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact_pair: &ContactPair,
        _total_force_magnitude: Real,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use crate::physics::*;

    #[test]
    fn test_from_user_data_with_negative() {
        let data = UserData {
            body_type: BodyType::Other,
            id: i64::MIN,
        };
        let number: u128 = data.into();
        let converted_data: UserData = number.into();
        assert_eq!(data, converted_data);
    }

    #[test]
    fn test_from_user_data_with_positive() {
        let data = UserData {
            body_type: BodyType::Tank,
            id: i64::MAX,
        };
        let number: u128 = data.into();
        let converted_data: UserData = number.into();
        assert_eq!(data, converted_data);
    }
}

pub fn start() -> UnboundedSender<PhysicsCommand> {
    let mut object_sizes = HashMap::new();
    object_sizes.insert(ObjectConstants::RedBarrage as i32, point![96.0, 32.0]);
    object_sizes.insert(ObjectConstants::YellowBarrage as i32, point![104.0, 32.0]);
    object_sizes.insert(ObjectConstants::RustyBarrelStay as i32, point![48.0, 48.0]);
    object_sizes.insert(ObjectConstants::RustyBarrelLay as i32, point![40.0, 56.0]);
    object_sizes.insert(ObjectConstants::StealHedgehog as i32, point![56.0, 56.0]);
    object_sizes.insert(ObjectConstants::WoodenHedgehog as i32, point![56.0, 56.0]);
    object_sizes.insert(ObjectConstants::LargeBush as i32, point![128.0, 128.0]);
    object_sizes.insert(ObjectConstants::SmallBush as i32, point![72.0, 72.0]);

    let mut bullet_sizes = HashMap::new();
    bullet_sizes.insert("1 (4)", vector![16.0, 28.0]);
    bullet_sizes.insert("4", vector![16.0, 52.0]);
    bullet_sizes.insert("2 (2)", vector![16.0, 36.0]);
    bullet_sizes.insert("2 (3)", vector![13.0, 29.0]);
    bullet_sizes.insert("3", vector![24.0, 32.0]);

    let mut gun_sizes = HashMap::new();
    gun_sizes.insert("1 (4)", vector![24.0, 60.0]);
    gun_sizes.insert("5", vector![28.0, 72.0]);
    gun_sizes.insert("2 (2)", vector![24.0, 60.0]);
    gun_sizes.insert("4", vector![28.0, 64.0]);
    gun_sizes.insert("3", vector![32.0, 60.0]);
    gun_sizes.insert("9", vector![36.0, 80.0]);

    let maps = load_maps("Maps").unwrap();
    let mut data = std::fs::read_to_string("Maps/MapObjects/MapObjects.polygons").unwrap();
    let map_objects = BodyEditorLoader::from_json(&data).unwrap();
    data = std::fs::read_to_string("Tanks/TanksBodies.polygons").unwrap();
    let bodies = BodyEditorLoader::from_json(&data).unwrap();
    data = std::fs::read_to_string("Tanks/Bullets.polygons").unwrap();
    let bullets = BodyEditorLoader::from_json(&data).unwrap();
    drop(data);
    let (send, mut recv) = unbounded_channel();

    let mut map = HashMap::new();
    std::thread::spawn(move || {
        let mut battles = Vec::new();
        let mut gen = rand::thread_rng();
        loop {
            for i in 0..battles.len() + 1 {
                match recv.try_recv() {
                    Ok(cmd) => match cmd {
                        PhysicsCommand::CreateMatch {
                            players: (player1, player2),
                        } => {
                            if !map.contains_key(&player1.0.id) && !map.contains_key(&player2.0.id)
                            {
                                if let (
                                    Ok::<WorldPlayer, _>(mut player1),
                                    Ok::<WorldPlayer, _>(mut player2),
                                ) = (player1.try_into(), player2.try_into())
                                {
                                    let (collision_send, collision_recv) = unbounded_channel();
                                    let event_handler = ChannelledEventCollector {
                                        collision_event_sender: collision_send,
                                    };
                                    let mut world = PhysicsWorld {
                                        bodies: RigidBodySet::new(),
                                        colliders: ColliderSet::new(),
                                        gravity: vector![0.0, 0.0],
                                        integration_parameters: IntegrationParameters::default(),
                                        physics_pipeline: PhysicsPipeline::new(),
                                        islands: IslandManager::new(),
                                        broad_phase: BroadPhase::new(),
                                        narrow_phase: NarrowPhase::new(),
                                        impulse_joints: ImpulseJointSet::new(),
                                        multibody_joints: MultibodyJointSet::new(),
                                        ccd_solver: CCDSolver::new(),
                                        hooks: Box::new(CustomPhysicsHooks),
                                        events: Box::new(event_handler),
                                    };
                                    //add physics objects
                                    let battle_map = &maps[gen.gen_range(0..maps.len())];
                                    attach_box(
                                        &mut world.bodies,
                                        &mut world.colliders,
                                        battle_map.width as f32 * SCALE_TO_PHYSICS,
                                        battle_map.height as f32 * SCALE_TO_PHYSICS,
                                    );
                                    for object in &battle_map.objects {
                                        //if body not exists it is not material (bushes, etc...)
                                        let name = object.id.to_string();
                                        if map_objects.body_exists(&name) {
                                            let mut position = Isometry::new(
                                                vector![
                                                    (object.x + object_sizes[&object.id].x / 2f32)
                                                        * SCALE_TO_PHYSICS,
                                                    (object.y + object_sizes[&object.id].y / 2f32)
                                                        * SCALE_TO_PHYSICS
                                                ],
                                                0.0,
                                            );
                                            position.append_rotation_wrt_center_mut(
                                                &UnitComplex::new(object.rotation.to_radians()),
                                            );

                                            let rigid_body = RigidBodyBuilder::fixed()
                                                .user_data(
                                                    UserData::new(BodyType::Other, 0i64).into(),
                                                )
                                                .position(position)
                                                .build();

                                            let collider = map_objects.create_collider(
                                                &name,
                                                object_sizes.get(&object.id).unwrap().x as f32
                                                    * object.scale
                                                    * SCALE_TO_PHYSICS,
                                            );

                                            let rigid_body_handle = world.bodies.insert(rigid_body);
                                            let handle = world.colliders.insert_with_parent(
                                                collider,
                                                rigid_body_handle,
                                                &mut world.bodies,
                                            );

                                            world
                                                .colliders
                                                .get_mut(handle)
                                                .unwrap()
                                                .set_position_wrt_parent(Isometry::new(
                                                    -object_sizes
                                                        .get(&object.id)
                                                        .unwrap()
                                                        .coords
                                                        .scale(SCALE_TO_PHYSICS)
                                                        / 2f32,
                                                    0.0,
                                                ));
                                        }
                                    }
                                    //add players
                                    let mut position = Isometry::new(
                                        vector![
                                            battle_map.width as f32 * SCALE_TO_PHYSICS / 2f32,
                                            battle_map.player1_y as f32 * SCALE_TO_PHYSICS
                                        ],
                                        0.0,
                                    );
                                    position.append_rotation_wrt_center_mut(&UnitComplex::new(
                                        0f32.to_radians(),
                                    ));
                                    let player1_body = RigidBodyBuilder::dynamic()
                                        .position(position)
                                        .linvel(vector![
                                            0.0,
                                            -player1.tank_info.characteristics.velocity
                                                * SCALE_TO_PHYSICS
                                        ])
                                        .ccd_enabled(true)
                                        .user_data(
                                            UserData::new(BodyType::Tank, player1.player.id).into(),
                                        )
                                        .build();

                                    let player1_collider = bodies.create_collider(
                                        &player1.tank.id.to_string(),
                                        player1.tank_info.graphics_info.tank_width as f32
                                            * SCALE_TO_PHYSICS,
                                    );
                                    let player1_body_handle = world.bodies.insert(player1_body);
                                    player1.handle = player1_body_handle;
                                    let handle = world.colliders.insert_with_parent(
                                        player1_collider,
                                        player1_body_handle,
                                        &mut world.bodies,
                                    );
                                    world
                                        .colliders
                                        .get_mut(handle)
                                        .unwrap()
                                        .set_position_wrt_parent(Isometry::new(
                                            -vector![
                                                player1.tank_info.graphics_info.tank_width as f32,
                                                player1.tank_info.graphics_info.tank_height as f32
                                            ] / 2f32
                                                * SCALE_TO_PHYSICS,
                                            0.0,
                                        ));

                                    let mut position = Isometry::new(
                                        vector![
                                            battle_map.width as f32 * SCALE_TO_PHYSICS / 2f32,
                                            battle_map.player2_y as f32 * SCALE_TO_PHYSICS
                                        ],
                                        0.0,
                                    );
                                    position.append_rotation_wrt_center_mut(&UnitComplex::new(
                                        180f32.to_radians(),
                                    ));
                                    let velocity = vector![
                                        player2.tank_info.characteristics.velocity
                                            * SCALE_TO_PHYSICS
                                            * position.rotation.cos_angle(),
                                        player2.tank_info.characteristics.velocity
                                            * SCALE_TO_PHYSICS
                                            * position.rotation.sin_angle()
                                    ];

                                    let player2_body = RigidBodyBuilder::dynamic()
                                        .position(position)
                                        .linvel(velocity)
                                        .ccd_enabled(true)
                                        .user_data(
                                            UserData::new(BodyType::Tank, player2.player.id).into(),
                                        )
                                        .build();

                                    let player2_collider = bodies.create_collider(
                                        &player2.tank.id.to_string(),
                                        player2.tank_info.graphics_info.tank_width as f32
                                            * SCALE_TO_PHYSICS,
                                    );
                                    let player2_body_handle = world.bodies.insert(player2_body);
                                    player2.handle = player2_body_handle;
                                    let handle = world.colliders.insert_with_parent(
                                        player2_collider,
                                        player2_body_handle,
                                        &mut world.bodies,
                                    );
                                    world
                                        .colliders
                                        .get_mut(handle)
                                        .unwrap()
                                        .set_position_wrt_parent(Isometry::new(
                                            -vector![
                                                player2.tank_info.graphics_info.tank_width as f32,
                                                player2.tank_info.graphics_info.tank_height as f32
                                            ] / 2f32
                                                * SCALE_TO_PHYSICS,
                                            0.0,
                                        ));

                                    player1.stats.hp = (player1.tank_info.characteristics.hp as f32
                                        * (1f32 + (player1.tank.level - 1) as f32 / 10f32))
                                        as i32;
                                    player2.stats.hp = (player2.tank_info.characteristics.hp as f32
                                        * (1f32 + (player2.tank.level - 1) as f32 / 10f32))
                                        as i32;
                                    player1.stats.damage =
                                        (player1.tank_info.characteristics.damage
                                            * (1f32 + (player1.tank.level - 1) as f32 / 10f32))
                                            as i32;
                                    player2.stats.damage =
                                        (player2.tank_info.characteristics.damage
                                            * (1f32 + (player2.tank.level - 1) as f32 / 10f32))
                                            as i32;
                                    player1.stats.cool_down =
                                        player1.tank_info.characteristics.reloading;
                                    player2.stats.cool_down =
                                        player2.tank_info.characteristics.reloading;

                                    //notify players
                                    //player1
                                    let game_packet = GamePacket {
                                        time_left: (MAX_BATTLE_TIME + WAIT_TIME) as u16,
                                        my_data: GamePlayerData {
                                            x: (battle_map.width / 2) as f32,
                                            y: battle_map.player1_y as f32,
                                            body_rotation: world
                                                .bodies
                                                .get(player2_body_handle)
                                                .unwrap()
                                                .position()
                                                .rotation
                                                .angle()
                                                .to_degrees(),
                                            gun_rotation: 0f32,
                                            hp: player1.stats.hp as u16,
                                            cool_down: player1.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                        opponent_data: GamePlayerData {
                                            x: (battle_map.width / 2) as f32,
                                            y: battle_map.player2_y as f32,
                                            body_rotation: world
                                                .bodies
                                                .get(player1_body_handle)
                                                .unwrap()
                                                .position()
                                                .rotation
                                                .angle()
                                                .to_degrees(),
                                            gun_rotation: 180f32,
                                            hp: player2.stats.hp as u16,
                                            cool_down: player2.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                    };
                                    let data = Packet::MapFoundResponse {
                                        wait_time: WAIT_TIME,
                                        map: battle_map.clone(),
                                        opponent_nick: player2.player.nickname.clone().unwrap(),
                                        opponent_tank: player2.tank.clone(),
                                        my_tank: player1.tank.clone(),
                                        initial_packet: game_packet,
                                    };
                                    let mut buf = Vec::new();
                                    let mut serializer = Serializer::new(&mut buf);
                                    data.serialize(&mut serializer).unwrap();
                                    if futures::executor::block_on(async {
                                        let mut uni = player1.conn.open_uni().await?;
                                        uni.write_all(&buf).await?;
                                        uni.finish().await?;
                                        Result::<()>::Ok(())
                                    })
                                    .is_err()
                                    {
                                        player1.connected = false;
                                    }

                                    //player2
                                    let game_packet = GamePacket {
                                        time_left: (MAX_BATTLE_TIME + WAIT_TIME) as u16,
                                        opponent_data: GamePlayerData {
                                            x: (battle_map.width / 2) as f32,
                                            y: battle_map.player1_y as f32,
                                            body_rotation: world
                                                .bodies
                                                .get(player2_body_handle)
                                                .unwrap()
                                                .position()
                                                .rotation
                                                .angle()
                                                .to_degrees(),
                                            gun_rotation: 0f32,
                                            hp: player1.stats.hp as u16,
                                            cool_down: player1.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                        my_data: GamePlayerData {
                                            x: (battle_map.width / 2) as f32,
                                            y: battle_map.player2_y as f32,
                                            body_rotation: world
                                                .bodies
                                                .get(player1_body_handle)
                                                .unwrap()
                                                .position()
                                                .rotation
                                                .angle()
                                                .to_degrees(),
                                            gun_rotation: 180f32,
                                            hp: player2.stats.hp as u16,
                                            cool_down: player2.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                    };
                                    let data = Packet::MapFoundResponse {
                                        wait_time: WAIT_TIME,
                                        map: battle_map.clone(),
                                        opponent_nick: player1.player.nickname.clone().unwrap(),
                                        opponent_tank: player1.tank.clone(),
                                        my_tank: player2.tank.clone(),
                                        initial_packet: game_packet,
                                    };
                                    let mut buf = Vec::new();
                                    let mut serializer = Serializer::new(&mut buf);
                                    data.serialize(&mut serializer).unwrap();
                                    if futures::executor::block_on(async {
                                        let mut uni = player2.conn.open_uni().await?;
                                        uni.write_all(&buf).await?;
                                        uni.finish().await?;
                                        Result::<()>::Ok(())
                                    })
                                    .is_err()
                                    {
                                        player2.connected = false;
                                    }

                                    let battle = Battle {
                                        world,
                                        map: battle_map,
                                        players: (player1, player2),
                                        step: Instant::now(),
                                        time: MAX_BATTLE_TIME + WAIT_TIME,
                                        collision_recv,
                                    };

                                    map.insert(battle.players.0.player.id, battles.len());
                                    map.insert(battle.players.1.player.id, battles.len());
                                    battles.push(battle);
                                }
                            }
                        }
                        PhysicsCommand::PlayerPacket { id, position } => {
                            if let Some(&index) = map.get(&id) {
                                let battle = &mut battles[index];
                                let player = if id == battle.players.0.player.id {
                                    &mut battle.players.0
                                } else {
                                    &mut battle.players.1
                                };
                                if battle.time <= MAX_BATTLE_TIME {
                                    //Body rotation
                                    let player_body =
                                        battle.world.bodies.get_mut(player.handle).unwrap();
                                    let back_angle =
                                        revert_angle_by_y(player_body.rotation().angle());

                                    let mut diff = 0f32;
                                    if position.body_rotation != 0f32 {
                                        diff =
                                            position.body_rotation - player_body.rotation().angle();
                                        diff = diff.rem_euclid(360f32.to_radians())
                                            - 180f32.to_radians();
                                    }
                                    let mut back_diff = 0f32;
                                    if position.body_rotation != 0f32 {
                                        back_diff = position.body_rotation - back_angle;
                                        back_diff = back_diff.rem_euclid(360f32.to_radians())
                                            - 180f32.to_radians();
                                    }
                                    let ang_vel = player
                                        .tank_info
                                        .characteristics
                                        .body_rotate_degrees
                                        .to_radians();

                                    player_body.set_angvel(0f32, true);
                                    if position.body_rotation != 0f32 {
                                        if diff.abs() < back_diff.abs() {
                                            let alpha = player_body.rotation().angle();
                                            let direction = direction_by_2_angles(
                                                alpha,
                                                position.body_rotation,
                                            );
                                            if diff.abs() <= ang_vel * UPDATE_TIME {
                                                player_body.set_angvel(0f32, true);
                                            } else {
                                                player_body.set_angvel(direction * ang_vel, true);
                                            }
                                        } else {
                                            let direction = direction_by_2_angles(
                                                back_angle,
                                                position.body_rotation,
                                            );
                                            if back_diff.abs() <= ang_vel * UPDATE_TIME {
                                                player_body.set_angvel(0f32, true);
                                            } else {
                                                player_body.set_angvel(direction * ang_vel, true);
                                            }
                                        }
                                    }
                                    if position.moving {
                                        if diff.abs() > back_diff.abs() {
                                            let velocity = vector![
                                                player.tank_info.characteristics.velocity
                                                    * SCALE_TO_PHYSICS
                                                    * (player_body.rotation().angle()
                                                        - 90f32.to_radians())
                                                    .cos(),
                                                player.tank_info.characteristics.velocity
                                                    * SCALE_TO_PHYSICS
                                                    * (player_body.rotation().angle()
                                                        - 90f32.to_radians())
                                                    .sin()
                                            ];
                                            player_body.set_linvel(velocity, true);
                                        } else {
                                            let velocity = vector![
                                                player.tank_info.characteristics.velocity
                                                    * 0.5f32
                                                    * SCALE_TO_PHYSICS
                                                    * (back_angle - 90f32.to_radians()).cos(),
                                                player.tank_info.characteristics.velocity
                                                    * 0.5f32
                                                    * SCALE_TO_PHYSICS
                                                    * (back_angle - 90f32.to_radians()).sin()
                                            ];
                                            player_body.set_linvel(velocity, true);
                                        }
                                    } else {
                                        player_body.set_linvel(Vector::zeros(), true);
                                    }

                                    //Gun rotation
                                    player.stats.gun_rotation = position.gun_rotation;
                                }
                            }
                        }
                        PhysicsCommand::PlayerShoot { id } => {
                            if let Some(&index) = map.get(&id) {
                                let battle = &mut battles[index];
                                let player = if id == battle.players.0.player.id {
                                    &mut battle.players.0
                                } else {
                                    &mut battle.players.1
                                };
                                if player.stats.cool_down == 0f32 && battle.time <= MAX_BATTLE_TIME
                                {
                                    player.stats.shots += 1;
                                    player.stats.cool_down =
                                        player.tank_info.characteristics.reloading;

                                    let mut point = *battle
                                        .world
                                        .bodies
                                        .get(player.handle)
                                        .unwrap()
                                        .translation();
                                    let gun_angle = player.stats.gun_angle
                                        + battle
                                            .world
                                            .bodies
                                            .get(player.handle)
                                            .unwrap()
                                            .rotation()
                                            .angle();

                                    point -= vector![
                                        player.tank_info.graphics_info.tank_width as f32,
                                        -(player.tank_info.graphics_info.tank_height as f32)
                                    ] / 2f32
                                        * SCALE_TO_PHYSICS;
                                    let size = gun_sizes
                                        .get(player.tank_info.graphics_info.tank_gun_name.as_str())
                                        .unwrap();
                                    let rotation_point = point
                                        + vector![
                                            (player.tank_info.graphics_info.gun_x
                                                + player.tank_info.graphics_info.gun_origin_x)
                                                as f32,
                                            -((player.tank_info.graphics_info.gun_y
                                                + player.tank_info.graphics_info.gun_origin_y)
                                                as f32)
                                        ] * SCALE_TO_PHYSICS;
                                    point += vector![
                                        player.tank_info.graphics_info.gun_x as f32 + size.x / 2f32,
                                        -(player.tank_info.graphics_info.gun_y as f32) - size.y
                                    ] * SCALE_TO_PHYSICS;
                                    let new_x = (point.x - rotation_point.x) * gun_angle.cos()
                                        - (point.y - rotation_point.y) * gun_angle.sin()
                                        + rotation_point.x;
                                    let new_y = (point.x - rotation_point.x) * gun_angle.sin()
                                        + (point.y - rotation_point.y) * gun_angle.cos()
                                        + rotation_point.y;
                                    point = vector![new_x, new_y];
                                    let mut position = Isometry::new(point, 0.0);
                                    position.append_rotation_wrt_center_mut(&UnitComplex::new(
                                        gun_angle,
                                    ));
                                    let velocity = vector![
                                        player.tank_info.characteristics.bullet_speed
                                            * SCALE_TO_PHYSICS
                                            * (position.rotation.angle() - 90f32.to_radians())
                                                .cos(),
                                        player.tank_info.characteristics.bullet_speed
                                            * SCALE_TO_PHYSICS
                                            * (position.rotation.angle() - 90f32.to_radians())
                                                .sin()
                                    ];

                                    let bullet_body = RigidBodyBuilder::dynamic()
                                        .position(position)
                                        .linvel(velocity)
                                        .ccd_enabled(true)
                                        .user_data(
                                            UserData::new(BodyType::Bullet, player.player.id)
                                                .into(),
                                        )
                                        .build();

                                    let bullet_collider = bullets.create_collider(
                                        &player.tank_info.graphics_info.bullet_name,
                                        bullet_sizes
                                            .get(
                                                player.tank_info.graphics_info.bullet_name.as_str(),
                                            )
                                            .unwrap()
                                            .x as f32
                                            * SCALE_TO_PHYSICS,
                                    );
                                    let bullet_body_handle =
                                        battle.world.bodies.insert(bullet_body);
                                    let handle = battle.world.colliders.insert_with_parent(
                                        bullet_collider,
                                        bullet_body_handle,
                                        &mut battle.world.bodies,
                                    );
                                    battle
                                        .world
                                        .colliders
                                        .get_mut(handle)
                                        .unwrap()
                                        .set_position_wrt_parent(Isometry::new(
                                            -bullet_sizes
                                                .get(
                                                    player
                                                        .tank_info
                                                        .graphics_info
                                                        .bullet_name
                                                        .as_str(),
                                                )
                                                .unwrap()
                                                / 2f32
                                                * SCALE_TO_PHYSICS,
                                            0.0,
                                        ));
                                }
                            }
                        }
                        PhysicsCommand::NotifyPlayerAboutMatch { id, new_conn } => {
                            if let Some(&index) = map.get(&id) {
                                let battle = &mut battles[index];
                                let player = if id == battle.players.0.player.id {
                                    battle.players.0.conn = new_conn;
                                    battle.players.0.connected = true;
                                    &battle.players.0
                                } else {
                                    battle.players.1.conn = new_conn;
                                    battle.players.1.connected = true;
                                    &battle.players.1
                                };
                                let op_player = if id != battle.players.0.player.id {
                                    &battle.players.0
                                } else {
                                    &battle.players.1
                                };

                                let my_pos =
                                    battle.world.bodies.get(player.handle).unwrap().position();
                                let op_pos = battle
                                    .world
                                    .bodies
                                    .get(op_player.handle)
                                    .unwrap()
                                    .position();

                                let game_packet = GamePacket {
                                    time_left: battle.time as u16,
                                    my_data: GamePlayerData {
                                        x: my_pos.translation.x * SCALE_TO_PIXELS,
                                        y: my_pos.translation.y * SCALE_TO_PIXELS,
                                        body_rotation: my_pos.rotation.angle().to_degrees(),
                                        gun_rotation: player.stats.gun_angle.to_degrees(),
                                        hp: player.stats.hp as u16,
                                        cool_down: player.stats.cool_down,
                                        bullets: Vec::new(),
                                    },
                                    opponent_data: GamePlayerData {
                                        x: op_pos.translation.x * SCALE_TO_PIXELS,
                                        y: op_pos.translation.y * SCALE_TO_PIXELS,
                                        body_rotation: op_pos.rotation.angle().to_degrees(),
                                        gun_rotation: op_player.stats.gun_angle.to_degrees(),
                                        hp: op_player.stats.hp as u16,
                                        cool_down: 0f32,
                                        bullets: Vec::new(),
                                    },
                                };
                                let data = Packet::MapFoundResponse {
                                    wait_time: 0f32.max(battle.time - MAX_BATTLE_TIME),
                                    map: battle.map.clone(),
                                    opponent_nick: op_player.player.nickname.clone().unwrap(),
                                    opponent_tank: op_player.tank.clone(),
                                    my_tank: player.tank.clone(),
                                    initial_packet: game_packet,
                                };
                                let mut buf = Vec::new();
                                let mut serializer = Serializer::new(&mut buf);
                                data.serialize(&mut serializer).unwrap();
                                if let Some(e) = futures::executor::block_on(async {
                                    let mut uni = player.conn.open_uni().await?;
                                    uni.write_all(&buf).await?;
                                    uni.finish().await?;
                                    Result::<()>::Ok(())
                                })
                                .err()
                                {
                                    tracing::warn!(
                                        "error occured while notification about battle {:?}",
                                        e
                                    );
                                }
                            }
                        }
                    },
                    Err(e) => {
                        if e == TryRecvError::Disconnected {
                            break;
                        }
                    }
                }
                if i >= battles.len() {
                    continue;
                }
                let step = battles[i].step.elapsed().as_secs_f32();
                if step >= UPDATE_TIME {
                    battles[i].step = Instant::now();
                    battles[i].time -= step;

                    //End battle
                    if battles[i].time <= 0f32
                        || battles[i].players.0.stats.hp == 0
                        || battles[i].players.1.stats.hp == 0
                    {
                        battles[i].players.0.player.battles_count += 1;
                        battles[i].players.1.player.battles_count += 1;

                        if battles[i].players.0.stats.hp == 0 {
                            let battle = &mut battles[i];
                            send_results!(battle, gen, 0, 1, false);
                        } else if battles[i].players.1.stats.hp == 0 {
                            let battle = &mut battles[i];
                            send_results!(battle, gen, 1, 0, false);
                        } else {
                            let battle = &mut battles[i];
                            send_results!(battle, gen, 1, 0, true);
                        }

                        map.remove(&battles[i].players.0.player.id);
                        map.remove(&battles[i].players.1.player.id);

                        if battles.len() - 1 != i {
                            *map.get_mut(&battles[battles.len() - 1].players.0.player.id)
                                .unwrap() = i;
                            *map.get_mut(&battles[battles.len() - 1].players.1.player.id)
                                .unwrap() = i;
                        }
                        let battle = battles.swap_remove(i);
                        std::thread::spawn(move || {
                            crate::db::update_player(&battle.players.0.player).unwrap();
                            crate::db::update_player(&battle.players.1.player).unwrap();
                        });
                        continue;
                    }

                    if battles[i].time >= MAX_BATTLE_TIME {
                        continue;
                    }
                    battles[i].world.integration_parameters.dt = step;
                    battles[i].world.integration_parameters.min_ccd_dt = step / 100f32;
                    battles[i].physics_step();
                    battles[i].players.0.stats.cool_down =
                        0f32.max(battles[i].players.0.stats.cool_down - step);
                    battles[i].players.1.stats.cool_down =
                        0f32.max(battles[i].players.1.stats.cool_down - step);

                    while let Ok((collision_event, mut point)) =
                        battles[i].collision_recv.try_recv()
                    {
                        // Handle the collision event.
                        if !collision_event.removed() && collision_event.started() {
                            point *= SCALE_TO_PIXELS;

                            let body1_handle = battles[i]
                                .world
                                .colliders
                                .get(collision_event.collider1())
                                .unwrap()
                                .parent()
                                .unwrap();
                            let body2_handle = battles[i]
                                .world
                                .colliders
                                .get(collision_event.collider2())
                                .unwrap()
                                .parent()
                                .unwrap();
                            let body1 = battles[i].world.bodies.get(body1_handle).unwrap();
                            let body2 = battles[i].world.bodies.get(body2_handle).unwrap();
                            let data1: UserData = body1.user_data.into();
                            let data2: UserData = body2.user_data.into();
                            if data1.body_type == BodyType::Bullet {
                                battles[i].remove_body(body1_handle);
                                let explosion = Packet::Explosion {
                                    x: point.x,
                                    y: point.y,
                                    hit: data2.body_type == BodyType::Tank,
                                };
                                let mut buf = Vec::new();
                                let mut serializer = Serializer::new(&mut buf);
                                explosion.serialize(&mut serializer).unwrap();
                                if futures::executor::block_on(async {
                                    let mut uni = battles[i].players.0.conn.open_uni().await?;
                                    uni.write_all(&buf).await?;
                                    uni.finish().await?;
                                    Result::<()>::Ok(())
                                })
                                .is_err()
                                {
                                    battles[i].players.0.connected = false;
                                }
                                if futures::executor::block_on(async {
                                    let mut uni = battles[i].players.1.conn.open_uni().await?;
                                    uni.write_all(&buf).await?;
                                    uni.finish().await?;
                                    Result::<()>::Ok(())
                                })
                                .is_err()
                                {
                                    battles[i].players.1.connected = false;
                                }
                                match data2.body_type {
                                    BodyType::Bullet => {
                                        battles[i].remove_body(body2_handle);
                                    }
                                    BodyType::Tank => {
                                        if battles[i].players.1.player.id == data2.id {
                                            let damage = battles[i]
                                                .players
                                                .0
                                                .stats
                                                .hp
                                                .min(battles[i].players.1.stats.damage);
                                            battles[i].players.0.stats.hp -= damage;
                                            battles[i].players.0.stats.damage_taken += damage;
                                            battles[i].players.1.stats.succeeded_shots += 1;
                                            battles[i].players.1.stats.damage_dealt += damage;
                                        }
                                        if battles[i].players.0.player.id == data2.id {
                                            let damage = battles[i]
                                                .players
                                                .1
                                                .stats
                                                .hp
                                                .min(battles[i].players.0.stats.damage);
                                            battles[i].players.1.stats.hp -= damage;
                                            battles[i].players.1.stats.damage_taken += damage;
                                            battles[i].players.0.stats.succeeded_shots += 1;
                                            battles[i].players.0.stats.damage_dealt += damage;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if data2.body_type == BodyType::Bullet {
                                battles[i].remove_body(body2_handle);
                                let explosion = Packet::Explosion {
                                    x: point.x,
                                    y: point.y,
                                    hit: data1.body_type == BodyType::Tank,
                                };
                                let mut buf = Vec::new();
                                let mut serializer = Serializer::new(&mut buf);
                                explosion.serialize(&mut serializer).unwrap();
                                if futures::executor::block_on(async {
                                    let mut uni = battles[i].players.0.conn.open_uni().await?;
                                    uni.write_all(&buf).await?;
                                    uni.finish().await?;
                                    Result::<()>::Ok(())
                                })
                                .is_err()
                                {
                                    battles[i].players.0.connected = false;
                                }
                                if futures::executor::block_on(async {
                                    let mut uni = battles[i].players.1.conn.open_uni().await?;
                                    uni.write_all(&buf).await?;
                                    uni.finish().await?;
                                    Result::<()>::Ok(())
                                })
                                .is_err()
                                {
                                    battles[i].players.1.connected = false;
                                }

                                match data1.body_type {
                                    BodyType::Bullet => {
                                        battles[i].remove_body(body1_handle);
                                    }
                                    BodyType::Tank => {
                                        if battles[i].players.1.player.id == data2.id {
                                            let damage = battles[i]
                                                .players
                                                .0
                                                .stats
                                                .hp
                                                .min(battles[i].players.1.stats.damage);
                                            battles[i].players.0.stats.hp -= damage;
                                            battles[i].players.0.stats.damage_taken += damage;
                                            battles[i].players.1.stats.succeeded_shots += 1;
                                            battles[i].players.1.stats.damage_dealt += damage;
                                        }
                                        if battles[i].players.0.player.id == data2.id {
                                            let damage = battles[i]
                                                .players
                                                .1
                                                .stats
                                                .hp
                                                .min(battles[i].players.0.stats.damage);
                                            battles[i].players.1.stats.hp -= damage;
                                            battles[i].players.1.stats.damage_taken += damage;
                                            battles[i].players.0.stats.succeeded_shots += 1;
                                            battles[i].players.0.stats.damage_dealt += damage;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    //notify players
                    //player1
                    let mut diff = 0f32;
                    let gun_angle = battles[i].players.0.stats.gun_angle
                        + battles[i]
                            .world
                            .bodies
                            .get(battles[i].players.0.handle)
                            .unwrap()
                            .rotation()
                            .angle();
                    if battles[i].players.0.stats.gun_rotation != 0f32 {
                        diff = battles[i].players.0.stats.gun_rotation - gun_angle;
                        diff += 180f32.to_radians();
                        diff = diff.rem_euclid(360f32.to_radians()) - 180f32.to_radians();
                    }
                    let ang_vel = battles[i]
                        .players
                        .0
                        .tank_info
                        .characteristics
                        .gun_rotate_degrees
                        .to_radians();

                    if battles[i].players.0.stats.gun_rotation != 0f32 {
                        let direction = diff.signum();
                        if diff.abs() >= ang_vel * step {
                            battles[i].players.0.stats.gun_angle += direction * ang_vel * step;
                        }
                    }

                    let mut diff = 0f32;
                    let gun_angle = battles[i].players.1.stats.gun_angle
                        + battles[i]
                            .world
                            .bodies
                            .get(battles[i].players.1.handle)
                            .unwrap()
                            .rotation()
                            .angle();
                    if battles[i].players.1.stats.gun_rotation != 0f32 {
                        diff = battles[i].players.1.stats.gun_rotation - gun_angle;
                        diff += 180f32.to_radians();
                        diff = diff.rem_euclid(360f32.to_radians()) - 180f32.to_radians();
                    }
                    let ang_vel = battles[i]
                        .players
                        .1
                        .tank_info
                        .characteristics
                        .gun_rotate_degrees
                        .to_radians();

                    if battles[i].players.1.stats.gun_rotation != 0f32 {
                        let direction = diff.signum();
                        if diff.abs() >= ang_vel * step {
                            battles[i].players.1.stats.gun_angle += direction * ang_vel * step;
                        }
                    }

                    let my_pos = *battles[i]
                        .world
                        .bodies
                        .get(battles[i].players.0.handle)
                        .unwrap()
                        .position();

                    let op_pos = *battles[i]
                        .world
                        .bodies
                        .get(battles[i].players.1.handle)
                        .unwrap()
                        .position();

                    let mut player1_bullets = Vec::new();
                    let mut player2_bullets = Vec::new();

                    for rigid_body_handle in battles[i].world.islands.active_dynamic_bodies() {
                        let rigid_body = battles[i].world.bodies.get(*rigid_body_handle).unwrap();
                        let data: UserData = rigid_body.user_data.into();
                        if data.body_type == BodyType::Bullet {
                            let bullet = BulletData {
                                x: rigid_body.translation().x * SCALE_TO_PIXELS,
                                y: rigid_body.translation().y * SCALE_TO_PIXELS,
                                rotation: rigid_body.rotation().angle().to_degrees(),
                            };
                            if data.id == battles[i].players.0.player.id {
                                player1_bullets.push(bullet);
                            } else if data.id == battles[i].players.1.player.id {
                                player2_bullets.push(bullet);
                            }
                        }
                    }

                    let game_packet = GamePacket {
                        time_left: battles[i].time as u16,
                        my_data: GamePlayerData {
                            x: my_pos.translation.x * SCALE_TO_PIXELS,
                            y: my_pos.translation.y * SCALE_TO_PIXELS,
                            body_rotation: my_pos.rotation.angle().to_degrees(),
                            gun_rotation: battles[i].players.0.stats.gun_angle.to_degrees(),
                            hp: battles[i].players.0.stats.hp as u16,
                            cool_down: battles[i].players.0.stats.cool_down,
                            bullets: player1_bullets.clone(),
                        },
                        opponent_data: GamePlayerData {
                            x: op_pos.translation.x * SCALE_TO_PIXELS,
                            y: op_pos.translation.y * SCALE_TO_PIXELS,
                            body_rotation: op_pos.rotation.angle().to_degrees(),
                            gun_rotation: battles[i].players.1.stats.gun_angle.to_degrees(),
                            hp: battles[i].players.1.stats.hp as u16,
                            cool_down: 0f32,
                            bullets: player2_bullets.clone(),
                        },
                    };
                    let mut buf = Vec::new();
                    let mut serializer = Serializer::new(&mut buf);
                    game_packet.serialize(&mut serializer).unwrap();
                    if battles[i]
                        .players
                        .0
                        .conn
                        .send_datagram(bytes::Bytes::from(buf))
                        .is_err()
                    {
                        battles[i].players.0.connected = false;
                    }

                    //player2
                    let game_packet = GamePacket {
                        time_left: battles[i].time as u16,
                        opponent_data: GamePlayerData {
                            x: my_pos.translation.x * SCALE_TO_PIXELS,
                            y: my_pos.translation.y * SCALE_TO_PIXELS,
                            body_rotation: my_pos.rotation.angle().to_degrees(),
                            gun_rotation: battles[i].players.0.stats.gun_angle.to_degrees(),
                            hp: battles[i].players.0.stats.hp as u16,
                            cool_down: 0f32,
                            bullets: player1_bullets,
                        },
                        my_data: GamePlayerData {
                            x: op_pos.translation.x * SCALE_TO_PIXELS,
                            y: op_pos.translation.y * SCALE_TO_PIXELS,
                            body_rotation: op_pos.rotation.angle().to_degrees(),
                            gun_rotation: battles[i].players.1.stats.gun_angle.to_degrees(),
                            hp: battles[i].players.1.stats.hp as u16,
                            cool_down: battles[i].players.1.stats.cool_down,
                            bullets: player2_bullets,
                        },
                    };
                    let mut buf = Vec::new();
                    let mut serializer = Serializer::new(&mut buf);
                    game_packet.serialize(&mut serializer).unwrap();
                    if battles[i]
                        .players
                        .1
                        .conn
                        .send_datagram(bytes::Bytes::from(buf))
                        .is_err()
                    {
                        battles[i].players.0.connected = false;
                    }
                }
            }
        }
    });
    send
}

fn direction_by_2_angles(alpha: f32, mut beta: f32) -> f32 {
    let delta = 360f32.to_radians() - alpha;
    beta += delta;
    beta = beta.rem_euclid(360f32.to_radians());
    if beta < 180f32.to_radians() {
        -1f32
    } else {
        1f32
    }
}

fn revert_angle_by_y(alpha: f32) -> f32 {
    if alpha == 0f32 || alpha == 180f32.to_radians() {
        return alpha;
    }
    if alpha > 0f32 {
        alpha - 180f32.to_radians()
    } else {
        alpha + 180f32.to_radians()
    }
}

fn attach_box(bodies: &mut RigidBodySet, colliders: &mut ColliderSet, width: f32, height: f32) {
    let rigid_body = RigidBodyBuilder::fixed()
        .user_data(UserData::new(BodyType::Other, 0i64).into())
        .build();
    let points = vec![
        point![0.0, 0.0],
        point![width, 0.0],
        point![width, 0.0],
        point![width, height],
        point![width, height],
        point![0.0, height],
        point![0.0, height],
        point![0.0, 0.0],
    ];
    let collider = ColliderBuilder::polyline(points, None)
        .active_events(ActiveEvents::COLLISION_EVENTS)
        .build();
    let box_handle = bodies.insert(rigid_body);
    colliders.insert_with_parent(collider, box_handle, bodies);
}

fn load_maps(path: &str) -> Result<Vec<Map>> {
    let mut res = Vec::new();
    if let Ok(mut dir) = std::fs::read_dir(path) {
        while let Some(Ok(entry)) = dir.next() {
            if entry.path().is_file() && entry.path().extension().map_or(false, |v| v == "json") {
                let content = std::fs::read(entry.path())?;
                let value: Map = serde_json::from_slice(&content)?;
                res.push(value);
            }
        }
    }
    Ok(res)
}

#[derive(Serialize, Deserialize)]
struct PhysicsWorld {
    gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    #[serde(skip)]
    physics_pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    #[serde(skip, default = "empty_hook")]
    hooks: Box<dyn PhysicsHooks>,
    #[serde(skip, default = "empty_hook")]
    events: Box<dyn EventHandler>,
}

fn empty_hook() -> Box<()> {
    Box::new(())
}

struct Battle<'a> {
    world: PhysicsWorld,
    players: (WorldPlayer<'a>, WorldPlayer<'a>),
    map: &'a Map,
    step: Instant,
    time: f32,
    collision_recv: UnboundedReceiver<(CollisionEvent, Point<Real>)>,
}

impl Battle<'_> {
    fn physics_step(&mut self) {
        self.world.physics_pipeline.step(
            &self.world.gravity,
            &self.world.integration_parameters,
            &mut self.world.islands,
            &mut self.world.broad_phase,
            &mut self.world.narrow_phase,
            &mut self.world.bodies,
            &mut self.world.colliders,
            &mut self.world.impulse_joints,
            &mut self.world.multibody_joints,
            &mut self.world.ccd_solver,
            &*self.world.hooks,
            &*self.world.events,
        );
    }

    fn remove_body(&mut self, handle: RigidBodyHandle) {
        self.world.bodies.remove(
            handle,
            &mut self.world.islands,
            &mut self.world.colliders,
            &mut self.world.impulse_joints,
            &mut self.world.multibody_joints,
            true,
        );
    }
}

#[derive(Debug)]
struct BodyEditorLoader {
    model: Model,
}

#[derive(Default, Debug)]
struct Model {
    rigid_bodies: HashMap<String, RigidBodyModel>,
}

#[derive(Default, Debug)]
struct RigidBodyModel {
    name: String,
    image_path: String,
    origin: Point<Real>,
    polygons: Vec<PolygonModel>,
    circles: Vec<CircleModel>,
}

#[derive(Default, Debug)]
struct PolygonModel {
    vertices: Vec<Point<Real>>,
}

#[derive(Default, Debug)]
struct CircleModel {
    center: Point<Real>,
    radius: f32,
}

impl BodyEditorLoader {
    // -------------------------------------------------------------------------
    // Json reading process
    // -------------------------------------------------------------------------
    fn read_rigid_body(json_value: &Value) -> Result<RigidBodyModel> {
        let mut rb_model = RigidBodyModel::default();
        rb_model.name = String::from_str(json_value["name"].as_str().unwrap())?;
        rb_model.image_path = String::from_str(json_value["imagePath"].as_str().unwrap())?;

        let origin = &json_value["origin"];
        rb_model.origin = point![
            origin["x"].as_f64().unwrap() as f32,
            origin["y"].as_f64().unwrap() as f32
        ];

        // polygons
        let polygons = &json_value["polygons"];
        for polygon in polygons.as_array().unwrap() {
            let mut polygon_model = PolygonModel::default();

            for vertex in polygon.as_array().unwrap() {
                let vec = point![
                    vertex["x"].as_f64().unwrap() as f32,
                    vertex["y"].as_f64().unwrap() as f32
                ];
                polygon_model.vertices.push(vec);
            }
            rb_model.polygons.push(polygon_model);
        }

        // circles
        let circles = &json_value["circles"];
        for circle in circles.as_array().unwrap() {
            let mut circle_model = CircleModel::default();

            circle_model.center = point![
                circle["cx"].as_f64().unwrap() as f32,
                circle["cy"].as_f64().unwrap() as f32
            ];
            circle_model.radius = circle["cr"].as_f64().unwrap() as f32;

            rb_model.circles.push(circle_model);
        }
        Ok(rb_model)
    }

    fn from_json(str: &str) -> Result<Self> {
        let mut model = Model::default();

        let map: Value = serde_json::from_str(str)?;

        let body_elem = &map["rigidBodies"];

        for bodies in body_elem.as_array().unwrap() {
            let body = Self::read_rigid_body(bodies)?;
            model.rigid_bodies.insert(body.name.to_owned(), body);
        }
        Ok(Self { model })
    }

    fn create_collider(&self, name: &str, scale: Real) -> Collider {
        let rb_model = self.model.rigid_bodies.get(name).unwrap();

        let mut shapes = Vec::new();
        //polygons
        for polygon in &rb_model.polygons {
            let vertices: Vec<_> = polygon
                .vertices
                .iter()
                .map(|f| {
                    let mut f = *f;
                    f.apply(|e| *e *= scale);
                    f
                })
                .collect();
            let shape = SharedShape::convex_hull(&vertices).unwrap();
            shapes.push((Isometry::<Real>::rotation(0f32), shape));
        }

        //circles
        for circle in &rb_model.circles {
            let shape = SharedShape::ball(circle.radius * scale);
            shapes.push((
                Isometry::<Real>::translation(circle.center.x * scale, circle.center.y * scale),
                shape,
            ));
        }

        ColliderBuilder::compound(shapes)
            .active_hooks(ActiveHooks::FILTER_CONTACT_PAIRS | ActiveHooks::FILTER_INTERSECTION_PAIR)
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .build()
    }

    fn body_exists(&self, name: &str) -> bool {
        self.model.rigid_bodies.contains_key(name)
    }
}
