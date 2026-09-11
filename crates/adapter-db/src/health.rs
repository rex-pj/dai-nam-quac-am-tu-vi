//! Inspecting the real PostgreSQL schema.
//!
//! The snapshot test pins **the SQL string we generate**; this module reads **what PostgreSQL
//! actually has**. The two checks differ in kind: the first catches mistakes while writing a
//! migration, the second catches a migration whose result differs from the intent — or
//! someone editing the schema by hand.
//!
//! Every query here is likewise built with the typed builder, even when reading `pg_catalog`.
//! System tables are no exception to the "no magic strings" rule.

use sea_orm::sea_query::{Expr, ExprTrait, Func, Query, SelectStatement};
use sea_orm::{ConnectionTrait, DeriveIden, Order};

use crate::connect::Db;
use crate::error::DbError;

// ── System table identifiers ─────────────────────────────────────────────────

#[derive(DeriveIden)]
enum PgExtensionCat {
    #[sea_orm(iden = "pg_extension")]
    Table,
    #[sea_orm(iden = "extname")]
    ExtName,
}

#[derive(DeriveIden)]
enum PgTypeCat {
    #[sea_orm(iden = "pg_type")]
    Table,
    #[sea_orm(iden = "typname")]
    TypName,
    #[sea_orm(iden = "oid")]
    Oid,
}

#[derive(DeriveIden)]
enum PgEnumCat {
    #[sea_orm(iden = "pg_enum")]
    Table,
    #[sea_orm(iden = "enumtypid")]
    EnumTypId,
    #[sea_orm(iden = "enumlabel")]
    EnumLabel,
    #[sea_orm(iden = "enumsortorder")]
    EnumSortOrder,
}

#[derive(DeriveIden)]
enum PgIndexesCat {
    #[sea_orm(iden = "pg_indexes")]
    Table,
    #[sea_orm(iden = "schemaname")]
    SchemaName,
    #[sea_orm(iden = "tablename")]
    TableName,
    #[sea_orm(iden = "indexname")]
    IndexName,
    #[sea_orm(iden = "indexdef")]
    IndexDef,
}

#[derive(DeriveIden)]
enum InfoSchema {
    #[sea_orm(iden = "information_schema")]
    Schema,
    #[sea_orm(iden = "columns")]
    Columns,
    #[sea_orm(iden = "table_name")]
    TableName,
    #[sea_orm(iden = "column_name")]
    ColumnName,
    #[sea_orm(iden = "is_generated")]
    IsGenerated,
    #[sea_orm(iden = "data_type")]
    DataType,
    #[sea_orm(iden = "udt_name")]
    UdtName,
}

#[derive(DeriveIden)]
enum PgFnIden {
    #[sea_orm(iden = "version")]
    Version,
    #[sea_orm(iden = "immutable_unaccent")]
    ImmutableUnaccent,
    #[sea_orm(iden = "to_tsvector")]
    ToTsvector,
}

#[derive(DeriveIden)]
enum PgTypeIden {
    #[sea_orm(iden = "regconfig")]
    RegConfig,
    #[sea_orm(iden = "text")]
    Text,
}

/// The single result column of every query here — one name, one declaration.
#[derive(DeriveIden)]
enum OutCol {
    #[sea_orm(iden = "value")]
    Value,
}

const OUT: &str = "value";
/// The public schema.
const PUBLIC_SCHEMA: &str = "public";
/// The value of `information_schema.columns.is_generated` when a column is generated.
const GENERATED_ALWAYS: &str = "ALWAYS";

// ── Execution ────────────────────────────────────────────────────────────────

async fn strings(db: &Db, select: SelectStatement) -> Result<Vec<String>, DbError> {
    // sea-orm 2 accepts a builder statement directly; no intermediate `Statement` needed.
    let rows = db
        .connection()
        .query_all(&select)
        .await
        .map_err(|e| db.query_err(e))?;
    rows.into_iter()
        .map(|row| {
            row.try_get::<String>("", OUT).map_err(|e| DbError::Decode {
                column: OUT,
                expected: "text",
                message: e.to_string(),
            })
        })
        .collect()
}

async fn one_string(db: &Db, select: SelectStatement, what: &str) -> Result<String, DbError> {
    strings(db, select)
        .await?
        .pop()
        .ok_or_else(|| DbError::Query(format!("{what} returned no rows")))
}

// ── The inspections ──────────────────────────────────────────────────────────

/// The server version, read through the same query path as real queries.
pub async fn server_version(db: &Db) -> Result<String, DbError> {
    let select = Query::select()
        .expr_as(Func::cust(PgFnIden::Version), OutCol::Value)
        .to_owned();
    one_string(db, select, "version()").await
}

/// The installed extensions.
pub async fn extensions(db: &Db) -> Result<Vec<String>, DbError> {
    let select = Query::select()
        .expr_as(Expr::col(PgExtensionCat::ExtName), OutCol::Value)
        .from(PgExtensionCat::Table)
        .order_by(PgExtensionCat::ExtName, Order::Asc)
        .to_owned();
    strings(db, select).await
}

/// The labels of an enum type, **in declaration order** — the order Postgres compares by.
pub async fn enum_labels(db: &Db, type_name: &str) -> Result<Vec<String>, DbError> {
    let select = Query::select()
        .expr_as(
            Expr::col((PgEnumCat::Table, PgEnumCat::EnumLabel)),
            OutCol::Value,
        )
        .from(PgEnumCat::Table)
        .inner_join(
            PgTypeCat::Table,
            Expr::col((PgTypeCat::Table, PgTypeCat::Oid))
                .eq(Expr::col((PgEnumCat::Table, PgEnumCat::EnumTypId))),
        )
        .and_where(Expr::col((PgTypeCat::Table, PgTypeCat::TypName)).eq(Expr::val(type_name)))
        .order_by((PgEnumCat::Table, PgEnumCat::EnumSortOrder), Order::Asc)
        .to_owned();
    strings(db, select).await
}

/// The index names of a table.
pub async fn indexes(db: &Db, table: &str) -> Result<Vec<String>, DbError> {
    let select = Query::select()
        .expr_as(Expr::col(PgIndexesCat::IndexName), OutCol::Value)
        .from(PgIndexesCat::Table)
        .and_where(Expr::col(PgIndexesCat::SchemaName).eq(Expr::val(PUBLIC_SCHEMA)))
        .and_where(Expr::col(PgIndexesCat::TableName).eq(Expr::val(table)))
        .order_by(PgIndexesCat::IndexName, Order::Asc)
        .to_owned();
    strings(db, select).await
}

/// The full definition of an index, as PostgreSQL itself reports it.
pub async fn index_definition(db: &Db, index: &str) -> Result<Option<String>, DbError> {
    let select = Query::select()
        .expr_as(Expr::col(PgIndexesCat::IndexDef), OutCol::Value)
        .from(PgIndexesCat::Table)
        .and_where(Expr::col(PgIndexesCat::SchemaName).eq(Expr::val(PUBLIC_SCHEMA)))
        .and_where(Expr::col(PgIndexesCat::IndexName).eq(Expr::val(index)))
        .to_owned();
    Ok(strings(db, select).await?.pop())
}

/// The generated columns of a table.
pub async fn generated_columns(db: &Db, table: &str) -> Result<Vec<String>, DbError> {
    let select = Query::select()
        .expr_as(Expr::col(InfoSchema::ColumnName), OutCol::Value)
        .from((InfoSchema::Schema, InfoSchema::Columns))
        .and_where(Expr::col(InfoSchema::TableName).eq(Expr::val(table)))
        .and_where(Expr::col(InfoSchema::IsGenerated).eq(Expr::val(GENERATED_ALWAYS)))
        .order_by(InfoSchema::ColumnName, Order::Asc)
        .to_owned();
    strings(db, select).await
}

/// The data type of a column. Returns `(data_type, udt_name)` — arrays and user-defined
/// types are only distinguishable by `udt_name`, and this schema uses both.
pub async fn column_type(
    db: &Db,
    table: &str,
    column: &str,
) -> Result<Option<(String, String)>, DbError> {
    let data = Query::select()
        .expr_as(Expr::col(InfoSchema::DataType), OutCol::Value)
        .from((InfoSchema::Schema, InfoSchema::Columns))
        .and_where(Expr::col(InfoSchema::TableName).eq(Expr::val(table)))
        .and_where(Expr::col(InfoSchema::ColumnName).eq(Expr::val(column)))
        .to_owned();
    let udt = Query::select()
        .expr_as(Expr::col(InfoSchema::UdtName), OutCol::Value)
        .from((InfoSchema::Schema, InfoSchema::Columns))
        .and_where(Expr::col(InfoSchema::TableName).eq(Expr::val(table)))
        .and_where(Expr::col(InfoSchema::ColumnName).eq(Expr::val(column)))
        .to_owned();
    let Some(data) = strings(db, data).await?.pop() else {
        return Ok(None);
    };
    let Some(udt) = strings(db, udt).await?.pop() else {
        return Ok(None);
    };
    Ok(Some((data, udt)))
}

/// Try `immutable_unaccent` — evidence the accent-folding search setup really works.
pub async fn try_immutable_unaccent(db: &Db, input: &str) -> Result<String, DbError> {
    let select = Query::select()
        .expr_as(
            Func::cust(PgFnIden::ImmutableUnaccent).arg(Func::cast_as(input, PgTypeIden::Text)),
            OutCol::Value,
        )
        .to_owned();
    one_string(db, select, "immutable_unaccent").await
}

/// Try `to_tsvector` with a given search configuration.
pub async fn try_tsvector(db: &Db, config: &str, input: &str) -> Result<String, DbError> {
    // `to_tsvector` returns a `tsvector`, which cannot be read straight into a string — cast
    // to text to get exactly the form Postgres reports, which is convenient to assert on.
    let select = Query::select()
        .expr_as(
            Func::cast_as(
                Func::cust(PgFnIden::ToTsvector)
                    .arg(Func::cast_as(config, PgTypeIden::RegConfig))
                    .arg(input),
                PgTypeIden::Text,
            ),
            OutCol::Value,
        )
        .to_owned();
    one_string(db, select, "to_tsvector").await
}

/// A broad snapshot, used by `dnqatv db health` and the data-quality page.
#[derive(Debug, Clone)]
pub struct DbHealth {
    pub server_version: String,
    pub extensions: Vec<String>,
    pub migrations_applied: usize,
    pub migrations_pending: usize,
}

pub async fn snapshot(db: &Db) -> Result<DbHealth, DbError> {
    let states = crate::migrate::status(db).await?;
    Ok(DbHealth {
        server_version: server_version(db).await?,
        extensions: extensions(db).await?,
        migrations_applied: states.iter().filter(|s| s.applied).count(),
        migrations_pending: states.iter().filter(|s| !s.applied).count(),
    })
}
