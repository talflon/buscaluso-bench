// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use rusqlite::types::FromSql;
use rusqlite::{named_params, Connection, ToSql};

use super::BenchResult;

const SCHEMA: &str = r#"
create table if not exists bench_session_info (
  session_id int not null,
  name text not null,
  value text not null,
  primary key (session_id, name));

create index if not exists bench_session_info_name_idx
  on bench_session_info (name, value);

create table if not exists bench_run (
  session_id int not null,
  bench text not null,
  duration real not null,
  found_at int,
  err text);

create index if not exists bench_run_bench_idx
  on bench_run (bench, session_id);

create index if not exists bench_run_session_idx
  on bench_run (session_id);
"#;

pub struct BenchDb {
    conn: Connection,
}

impl BenchDb {
    pub fn new(conn: Connection) -> rusqlite::Result<BenchDb> {
        conn.execute_batch(SCHEMA)?;
        Ok(BenchDb { conn })
    }

    pub fn new_session_id(&mut self) -> rusqlite::Result<BenchSessionId> {
        let mut query = self.conn.prepare_cached(
            r#"
            select 1
              from bench_session_info
              where session_id = ?
              limit 1
            "#,
        )?;
        let mut session_id = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        while query.exists([session_id])? {
            session_id += 1;
        }
        Ok(BenchSessionId(session_id))
    }

    pub fn add_result(
        &mut self,
        session_id: BenchSessionId,
        bench: &str,
        result: BenchResult,
    ) -> rusqlite::Result<()> {
        let (found_at, err): (Option<usize>, Option<&str>) = match &result.found_index {
            Ok(found_at) => (*found_at, None),
            Err(err) => (None, Some(err)),
        };
        self.conn
            .prepare_cached(
                r#"
                insert into bench_run
                  (session_id, bench, duration, found_at, err)
                  values(?, ?, ?, ?, ?)
                "#,
            )?
            .execute((
                session_id,
                bench,
                result.elapsed.as_secs_f64(),
                found_at,
                err,
            ))?;
        Ok(())
    }

    pub fn get_results(
        &mut self,
        session_id: BenchSessionId,
        bench: &str,
    ) -> rusqlite::Result<Vec<BenchResult>> {
        let mut results = Vec::new();
        let mut stmt = self.conn.prepare_cached(
            r#"
            select duration, found_at, err
              from bench_run
              where session_id = ?
                and bench = ?
            "#,
        )?;
        let mut rows = stmt.query((session_id, bench))?;
        while let Some(row) = rows.next()? {
            let err: Option<String> = row.get(2)?;
            results.push(BenchResult {
                found_index: match err {
                    Some(err) => Err(err),
                    None => Ok(row.get(1)?),
                },
                elapsed: Duration::from_secs_f64(row.get(0)?),
            })
        }
        Ok(results)
    }

    pub fn set_info(
        &mut self,
        session_id: BenchSessionId,
        name: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        assert_eq!(
            self.conn
                .prepare_cached(
                    r#"
                    insert into bench_session_info
                      (session_id, name, value)
                      values(:session_id, :name, :value)
                      on conflict do update set
                        value = :value
                        where session_id = :session_id
                          and name = :name
                    "#,
                )?
                .execute(named_params! {
                    ":session_id": session_id,
                    ":name": name,
                    ":value": value,
                })?,
            1
        );
        Ok(())
    }

    pub fn get_info(
        &mut self,
        session_id: BenchSessionId,
    ) -> rusqlite::Result<BTreeMap<String, String>> {
        let mut map = BTreeMap::new();
        let mut stmt = self.conn.prepare_cached(
            r#"
            select name, value
              from bench_session_info
              where session_id = ?
            "#,
        )?;
        let mut rows = stmt.query([session_id])?;
        while let Some(row) = rows.next()? {
            map.insert(row.get(0)?, row.get(1)?);
        }
        Ok(map)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BenchSessionId(u64);

impl BenchSessionId {
    pub fn start_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.0)
    }
}

impl ToSql for BenchSessionId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for BenchSessionId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        u64::column_result(value).map(BenchSessionId)
    }
}
