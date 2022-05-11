use std::io::Write;

use diesel::{serialize, pg::Pg, deserialize, sql_types, types::{ToSql, FromSql, IsNull}};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
#[derive(Debug, FromSqlRow, AsExpression)]
#[sql_type = "DbTank"]
pub struct Tank {
    pub id: i32,

    pub level: i32,

    pub count: i32,
}

/// Handle to SQL type
#[derive(Debug, SqlType)]
#[postgres(type_name = "tank")]
pub struct DbTank;

impl ToSql<DbTank, Pg> for Tank {
    fn to_sql<W: Write>(&self, out: &mut serialize::Output<W, Pg>) -> serialize::Result {
        ToSql::<sql_types::Integer, Pg>::to_sql(&3, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.id, out)?;
        
        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.level, out)?;

        ToSql::<sql_types::Oid, Pg>::to_sql(&23, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&4, out)?;
        ToSql::<sql_types::Integer, Pg>::to_sql(&self.count, out)?;
        Ok(IsNull::No)
    }
}

impl FromSql<DbTank, Pg> for Tank {
    fn from_sql(bytes: Option<&[u8]>) -> deserialize::Result<Self> {
        let bytes = not_none!(bytes);

        let id = &bytes[12..16];
        let level = &bytes[24..28];
        let count = &bytes[36..40];

        Ok(Tank{
            id: FromSql::<sql_types::Integer, Pg>::from_sql(Some(id))?,
            level: FromSql::<sql_types::Integer, Pg>::from_sql(Some(level))?,
            count: FromSql::<sql_types::Integer, Pg>::from_sql(Some(count))?,
        })
    }
}
