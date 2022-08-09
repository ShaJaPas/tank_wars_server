use std::{collections::HashMap, str::FromStr};

use minstant::Instant;
use quinn::Connection;
use rand::Rng;
use rapier2d::{na::UnitComplex, prelude::*};
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::{error::TryRecvError, unbounded_channel, UnboundedSender};

use crate::data::{Map, Player, Tank, TankInfo, TANKS, GamePacket, Packet, GamePlayerData};

type Result<T> = color_eyre::Result<T>;

const UPDATE_TIME: f32 = 1f32 / 30f32;
const WAIT_TIME: f32 = 5f32;
const MAX_BATTLE_TIME: f32 = 60f32 * 3f32;
const SCALE_TO_PHYSICS: f32 = 1f32 / 50f32;
const SCALE_TO_PIXELS: f32 = 50f32;

#[derive(Debug)]
pub struct BalancedPlayer(pub Box<Player>, pub i32, pub Connection);

struct WorldPlayer<'a> {
    tank_info: &'a TankInfo,
    tank: Tank,
    player: Box<Player>,
    conn: Connection,
    stats: PlayerInfo,
}

#[derive(Default)]
struct PlayerInfo {
    damage_dealt: i32,
    damage_taken: i32,
    shots: i32,
    succeeded_shots: i32,
    gun_rotation: f32,
    hp: i32,
    damage: i32,
    gun_angle: f32,
    body_angle: f32,
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
        })
    }
}
#[derive(Debug)]
pub enum PhysicsCommand {
    CreateMatch {
        players: (BalancedPlayer, BalancedPlayer),
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
    let mut free_indexes = Vec::<usize>::new();
    std::thread::spawn(move || {
        let mut battles = Vec::new();
        let mut gen = rand::thread_rng();
        let mut time_step = Instant::now();
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
                                    Ok::<WorldPlayer, _>(player1),
                                    Ok::<WorldPlayer, _>(mut player2),
                                ) = (player1.try_into(), player2.try_into())
                                {
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
                                        hooks: Box::new(()),
                                        events: Box::new(()),
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
                                                    object.x * SCALE_TO_PHYSICS,
                                                    object.y * SCALE_TO_PHYSICS
                                                ],
                                                0.0,
                                            );
                                            position.append_rotation_wrt_center_mut(
                                                &UnitComplex::new(object.rotation.to_radians()),
                                            );

                                            let rigid_body = RigidBodyBuilder::fixed()
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
                                                        .scale(SCALE_TO_PHYSICS),
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
                                        90f32.to_radians(),
                                    ));
                                    let player1_body =
                                        RigidBodyBuilder::dynamic().position(position).linvel(vector![0.0, player1.tank_info.characteristics.velocity * SCALE_TO_PHYSICS]).build();
                                        
                                    let player1_collider = bodies.create_collider(
                                        &player1.tank.id.to_string(),
                                        player1.tank_info.graphics_info.tank_width as f32
                                            * SCALE_TO_PHYSICS,
                                    );
                                    let player1_body_handle = world.bodies.insert(player1_body);
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
                                    let player2_body =
                                        RigidBodyBuilder::dynamic().position(position).linvel(vector![0.0, player2.tank_info.characteristics.velocity * SCALE_TO_PHYSICS]).build();

                                    let player2_collider = bodies.create_collider(
                                        &player2.tank.id.to_string(),
                                        player2.tank_info.graphics_info.tank_width as f32
                                            * SCALE_TO_PHYSICS,
                                    );
                                    let player2_body_handle = world.bodies.insert(player2_body);
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
                                    player2.stats.body_angle = 180f32;
                                    player2.stats.gun_angle = 180f32;
                                    player2.stats.gun_rotation = 180f32;

                                    /*let mut buf = Vec::new();
                                    let mut serializer = rmp_serde::Serializer::new(&mut buf);
                                    world.serialize(&mut serializer).unwrap();
                                    std::fs::write(
                                        "/home/konstantin/Desktop/rapier_test/world.physics",
                                        buf,
                                    )
                                    .unwrap();*/

                                    //notify players
                                    //player1
                                    let game_packet = GamePacket {
                                        time_left: (MAX_BATTLE_TIME + WAIT_TIME) as u16,
                                        my_data: GamePlayerData{
                                            x: (battle_map.width / 2) as f32,
                                            y: (battle_map.player1_y / 2) as f32,
                                            body_rotation: 0f32,
                                            gun_rotation: 0f32,
                                            hp: player1.tank_info.characteristics.hp as u16,
                                            cool_down: player1.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                        opponnet_data: GamePlayerData{
                                            x: (battle_map.width / 2) as f32,
                                            y: (battle_map.player2_y / 2) as f32,
                                            body_rotation: 180f32,
                                            gun_rotation: 180f32,
                                            hp: player2.tank_info.characteristics.hp as u16,
                                            cool_down: player2.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                    };
                                    let data = Packet::MapFoundResponse{
                                        wait_time: WAIT_TIME,
                                        map: battle_map.clone(),
                                        opponent_nick: player2.player.nickname.clone().unwrap(),
                                        opponent_tank: player2.tank.clone(),
                                        initial_packet: game_packet,
                                    };
                                    let mut buf = Vec::new();
                                    let mut serializer = Serializer::new(&mut buf);
                                    data.serialize(&mut serializer).unwrap();
                                    futures::executor::block_on(async{
                                        let mut uni = player1.conn.open_uni().await?;
                                        uni.write_all(&buf).await?;
                                        uni.finish().await?;
                                        Result::<()>::Ok(())
                                    }).unwrap();

                                    //player2
                                    let game_packet = GamePacket {
                                        time_left: (MAX_BATTLE_TIME + WAIT_TIME) as u16,
                                        opponnet_data: GamePlayerData{
                                            x: (battle_map.width / 2) as f32,
                                            y: (battle_map.player1_y / 2) as f32,
                                            body_rotation: 0f32,
                                            gun_rotation: 0f32,
                                            hp: player1.tank_info.characteristics.hp as u16,
                                            cool_down: player1.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                        my_data: GamePlayerData{
                                            x: (battle_map.width / 2) as f32,
                                            y: (battle_map.player2_y / 2) as f32,
                                            body_rotation: 180f32,
                                            gun_rotation: 180f32,
                                            hp: player2.tank_info.characteristics.hp as u16,
                                            cool_down: player2.tank_info.characteristics.reloading,
                                            bullets: Vec::new(),
                                        },
                                    };
                                    let data = Packet::MapFoundResponse{
                                        wait_time: WAIT_TIME,
                                        map: battle_map.clone(),
                                        opponent_nick: player1.player.nickname.clone().unwrap(),
                                        opponent_tank: player1.tank.clone(),
                                        initial_packet: game_packet,
                                    };
                                    let mut buf = Vec::new();
                                    let mut serializer = Serializer::new(&mut buf);
                                    data.serialize(&mut serializer).unwrap();
                                    futures::executor::block_on(async{
                                        let mut uni = player2.conn.open_uni().await?;
                                        uni.write_all(&buf).await?;
                                        uni.finish().await?;
                                        Result::<()>::Ok(())
                                    }).unwrap();
                                    
                                    let battle = Battle {
                                        world,
                                        players: (player1, player2),
                                        battle_time: 0f32,
                                    };

                                    if let Some(x) = free_indexes.pop() {
                                        map.insert(battle.players.0.player.id, x);
                                        map.insert(battle.players.1.player.id, x);
                                        battles[x] = battle;
                                    } else {
                                        map.insert(battle.players.0.player.id, battles.len());
                                        map.insert(battle.players.1.player.id, battles.len());
                                        battles.push(battle);
                                    }
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
                if i == battles.len() {
                    continue;
                }
                let step = time_step.elapsed().as_secs_f32();
                if  step >= UPDATE_TIME {
                    battles[i].battle_time += step;
                    if battles[i].battle_time >= WAIT_TIME {
                        battles[i].physics_step();

                    }
                    time_step = Instant::now();
                }
            }
        }
    });
    send
}

fn attach_box(bodies: &mut RigidBodySet, colliders: &mut ColliderSet, width: f32, height: f32) {
    let rigid_body = RigidBodyBuilder::fixed().build();
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
    let collider = ColliderBuilder::polyline(points, None).build();
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
    return Box::new(());
}

struct Battle<'a> {
    world: PhysicsWorld,
    players: (WorldPlayer<'a>, WorldPlayer<'a>),
    battle_time: f32,
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
                    let mut f = f.clone();
                    f.apply(|e| *e = *e * scale);
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

        ColliderBuilder::compound(shapes).build()
    }

    fn body_exists(&self, name: &str) -> bool {
        self.model.rigid_bodies.contains_key(name)
    }
}
