table! {
    use diesel::sql_types::*;
    use crate::data::*;

    players (id) {
        id -> Int8,
        machine_id -> Varchar,
        reg_date -> Timestamp,
        last_online -> Timestamp,
        nickname -> Nullable<Varchar>,
        battles_count -> Int4,
        victories_count -> Int4,
        xp -> Int4,
        rank_level -> Int4,
        coins -> Int4,
        diamonds -> Int4,
        daily_items_time -> Timestamp,
        friends_nicks -> Array<Text>,
        accuracy -> Float4,
        damage_dealt -> Int4,
        damage_taken -> Int4,
        trophies -> Int4,
        tanks -> Array<DbTank>,
        daily_items -> Array<DbDailyItem>,
    }
}
