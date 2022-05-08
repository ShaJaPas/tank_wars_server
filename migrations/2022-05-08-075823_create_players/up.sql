CREATE TYPE "tank" AS (
    "id" INTEGER,
    "level" INTEGER,
    "count" INTEGER
);

CREATE TYPE "daily_item" AS (
    "price" INTEGER,
    "tank_id" INTEGER,
    "count" INTEGER,
    "bought" BOOLEAN
);

CREATE TABLE "players" (
    "id" BIGINT PRIMARY KEY NOT NULL, 
    "machine_id" VARCHAR(20) NOT NULL, 
    "reg_date" TIMESTAMP NOT NULL, 
    "last_online" TIMESTAMP NOT NULL, 
    "nickname" VARCHAR(20) UNIQUE, 
    "battles_count" INTEGER NOT NULL, 
    "victories_count" INTEGER NOT NULL, 
    "xp" INTEGER NOT NULL, 
    "coins" INTEGER NOT NULL, 
    "diamonds" INTEGER NOT NULL, 
    "daily_items_time" TIMESTAMP NOT NULL, 
    "friends_nicks" VARCHAR(20) [] NOT NULL, 
    "accuracy" FLOAT NOT NULL, 
    "damage_dealt" INTEGER NOT NULL, 
    "damage_taken" INTEGER NOT NULL, 
    "trophies" INTEGER NOT NULL, 
    "tanks" tank[] NOT NULL,
    "daily_items" daily_item[] NOT NULL
);
