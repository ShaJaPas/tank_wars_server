use rand::Rng;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use super::{Player, Tank, TankRarity, WeightedRandomList};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Chest {
    pub name: ChestName,

    pub loot: Vec<Tank>,

    pub coins: u32,

    pub diamonds: u32,
}

impl Chest {
    pub fn generate_random_loot(name: ChestName, player: &Player) -> Chest {
        let mut chest = Chest::default();
        let mut rng = rand::thread_rng();
        let tanks = super::TANKS.get();
        match name {
            ChestName::COMMON => {
                chest.coins = rng.gen_range(20..=40);
                chest.diamonds = rng.gen_range(0..=4);
                let mut list = WeightedRandomList::new();
                for x in tanks.iter().map(|f| {
                    (
                        f,
                        if player.tanks.iter().any(|t| t.id == f.id as i32) {
                            70.0
                        } else {
                            f.characteristics.rarity.value()
                        },
                    )
                }) {
                    list.add_entry(x.0, x.1);
                }
                let mut loot = Vec::new();
                for _ in 0..rng.gen_range(2..=3) {
                    let tank = list.get_random().unwrap();
                    let chance = list.remove_enty(tank).unwrap();
                    loot.push((
                        Tank {
                            id: tank.id as i32,
                            level: 0,
                            count: if chance == 70.0 {
                                rng.gen_range(30..=50)
                            } else {
                                0
                            },
                        },
                        chance,
                        &tank.characteristics.rarity,
                    ));
                }
                loot.sort_by(|a, b| {
                    if a.1.total_cmp(&b.1).is_eq() {
                        b.2.cmp(a.2)
                    } else {
                        a.1.total_cmp(&b.1)
                    }
                });
                chest.loot = loot.into_iter().map(|f| f.0).collect();
            }
            ChestName::STARTER => {
                chest.coins = rng.gen_range(40..=60);
                chest.diamonds = rng.gen_range(2..=5);
                let mut list = WeightedRandomList::new();
                for x in tanks.iter().map(|f| {
                    (
                        f,
                        if player.tanks.iter().any(|t| t.id == f.id as i32) {
                            70.0
                        } else if f.characteristics.rarity != TankRarity::COMMON {
                            f.characteristics.rarity.value() * 2.0
                        } else {
                            f.characteristics.rarity.value()
                        },
                    )
                }) {
                    list.add_entry(x.0, x.1);
                }
                let mut loot = Vec::new();
                for _ in 0..rng.gen_range(1..=2) {
                    let tank = list.get_random().unwrap();
                    let chance = list.remove_enty(tank).unwrap();
                    loot.push((
                        Tank {
                            id: tank.id as i32,
                            level: 0,
                            count: if chance == 70.0 {
                                rng.gen_range(5..=7)
                            } else {
                                0
                            },
                        },
                        chance,
                        &tank.characteristics.rarity,
                    ));
                }
                loot.sort_by(|a, b| {
                    if a.1.total_cmp(&b.1).is_eq() {
                        b.2.cmp(a.2)
                    } else {
                        a.1.total_cmp(&b.1)
                    }
                });
                chest.loot = loot.into_iter().map(|f| f.0).collect();
            }
            _ => unimplemented!(),
        }
        chest.name = name;
        chest
    }

    pub fn add_to_player(&self, player: &mut Player) {
        player.coins += self.coins as i32;
        player.diamonds += self.diamonds as i32;
        for x in &self.loot {
            if let Some((i, _)) = player.tanks.iter().enumerate().find(|f| f.1.id == x.id) {
                player.tanks[i].count += x.count;
            } else {
                player.tanks.push(Tank {
                    id: x.id,
                    level: 1,
                    count: x.count,
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, EnumIter, Default)]
pub enum ChestName {
    #[default]
    STARTER = 0,
    COMMON = 100,
    RARE = 240,
    EPIC = 350,
    MYTHICAL = 500,
    LEGENDARY = 1000,
}
