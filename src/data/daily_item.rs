use std::io::Write;

use diesel::{
    deserialize,
    pg::Pg,
    serialize, sql_types,
    types::{FromSql, IsNull, ToSql},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug, FromSqlRow, AsExpression)]
#[sql_type = "DbDailyItem"]
pub struct DailyItem {
    pub price: i32,
    pub tank_id: i32,
    pub count: i32,
    pub bought: bool,
}

/// Handle to SQL type
#[derive(Debug, SqlType)]
#[postgres(type_name = "daily_item")]
pub struct DbDailyItem;

impl ToSql<DbDailyItem, Pg> for DailyItem {
    fn to_sql<W: Write>(&self, out: &mut serialize::Output<W, Pg>) -> serialize::Result {
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.price, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.tank_id, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.count, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&16, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&1, out)?;
        ToSql::<sql_types::Bool, Pg>::to_sql(&self.bought, out)?;
        Ok(IsNull::No)
    }
}

impl FromSql<DbDailyItem, Pg> for DailyItem {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let bytes = not_none!(bytes);

        let price = &bytes[12..16];
        let tank_id = &bytes[24..28];
        let count = &bytes[36..40];
        let bought = &bytes[48..49];

        Ok(DailyItem {
            price: FromSql::<sql_types::Integer, Pg>::from_sql(Some(price))?,
            tank_id: FromSql::<sql_types::Integer, Pg>::from_sql(Some(tank_id))?,
            count: FromSql::<sql_types::Integer, Pg>::from_sql(Some(count))?,
            bought: FromSql::<sql_types::Bool, Pg>::from_sql(Some(bought))?,
        })
    }
}
