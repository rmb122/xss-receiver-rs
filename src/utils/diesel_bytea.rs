use diesel::deserialize::FromSqlRow;
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::sql_types;
use diesel::{deserialize::FromSql, serialize::ToSql};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

#[derive(FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
#[diesel(sql_type = sql_types::Binary)]
pub struct Json<T: Sized>(pub T);

impl<T> Json<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> AsRef<T> for Json<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for Json<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> FromSql<sql_types::Binary, Pg> for Json<T>
where
    T: std::fmt::Debug + DeserializeOwned,
{
    fn from_sql(bytes: PgValue) -> diesel::deserialize::Result<Self> {
        let value = <Vec<u8> as FromSql<sql_types::Binary, Pg>>::from_sql(bytes)?;
        Ok(Self(serde_json::from_slice::<T>(&value)?))
    }
}

impl<T> ToSql<sql_types::Binary, Pg> for Json<T>
where
    T: std::fmt::Debug + Serialize,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        let value = serde_json::to_vec(&self.0)?;
        <Vec<u8> as ToSql<sql_types::Binary, Pg>>::to_sql(&value, &mut out.reborrow())
    }
}

impl<T> PartialEq for Json<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Default for Json<T>
where
    T: Default,
{
    fn default() -> Self {
        Self(T::default())
    }
}

#[derive(
    FromSqlRow, AsExpression, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default,
)]
#[serde(transparent)]
#[diesel(sql_type = sql_types::Binary)]
pub struct StringBytes(pub String);

impl StringBytes {
    pub fn new(value: String) -> Self {
        Self(value)
    }
}

impl Deref for StringBytes {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StringBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FromSql<sql_types::Binary, Pg> for StringBytes {
    fn from_sql(bytes: PgValue) -> diesel::deserialize::Result<Self> {
        let value = <Vec<u8> as FromSql<sql_types::Binary, Pg>>::from_sql(bytes)?;
        Ok(Self(String::from_utf8(value)?))
    }
}

impl ToSql<sql_types::Binary, Pg> for StringBytes {
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, Pg>,
    ) -> diesel::serialize::Result {
        <Vec<u8> as ToSql<sql_types::Binary, Pg>>::to_sql(
            &self.0.as_bytes().to_vec(),
            &mut out.reborrow(),
        )
    }
}
