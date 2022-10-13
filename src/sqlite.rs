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
create table if not exists bench_run_info (
  start_time int not null,
  name text not null,
  value text not null,
  primary key (start_time, name));

create index if not exists bench_run_info_name_idx
  on bench_run_info (name, value);

create table if not exists bench_run_item (
  start_time int not null,
  bench text not null,
  duration real not null,
  found_at int,
  err text);

create index if not exists bench_run_item_bench_idx
  on bench_run_item (bench, start_time);
"#;

pub struct BenchDb {
    conn: Connection,
}

impl BenchDb {
    pub fn new(conn: Connection) -> rusqlite::Result<BenchDb> {
        conn.execute_batch(SCHEMA)?;
        Ok(BenchDb { conn })
    }

    pub fn new_start(&mut self) -> rusqlite::Result<BenchRunStart> {
        let mut query = self.conn.prepare_cached(
            r#"
            select 1
              from bench_run_info
              where start_time = ?
              limit 1
            "#,
        )?;
        let mut start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        while query.exists([start_time])? {
            start_time += 1;
        }
        Ok(BenchRunStart(start_time))
    }

    pub fn add_result(
        &mut self,
        start: BenchRunStart,
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
                insert into bench_run_item
                  (start_time, bench, duration, found_at, err)
                  values(?, ?, ?, ?, ?)
                "#,
            )?
            .execute((start, bench, result.elapsed.as_secs_f64(), found_at, err))?;
        Ok(())
    }

    pub fn get_results(
        &mut self,
        start: BenchRunStart,
        bench: &str,
    ) -> rusqlite::Result<Vec<BenchResult>> {
        let mut results = Vec::new();
        let mut stmt = self.conn.prepare_cached(
            r#"
            select duration, found_at, err
              from bench_run_item
              where start_time = ?
                and bench = ?
            "#,
        )?;
        let mut rows = stmt.query((start, bench))?;
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
        start: BenchRunStart,
        name: &str,
        value: &str,
    ) -> rusqlite::Result<()> {
        assert_eq!(
            self.conn
                .prepare_cached(
                    r#"
                    insert into bench_run_info
                      (start_time, name, value)
                      values(:start, :name, :value)
                      on conflict do update set
                        value = :value
                        where start_time = :start
                          and name = :name
                    "#,
                )?
                .execute(named_params! { ":start": start, ":name": name, ":value": value })?,
            1
        );
        Ok(())
    }

    pub fn get_info(&mut self, start: BenchRunStart) -> rusqlite::Result<BTreeMap<String, String>> {
        let mut map = BTreeMap::new();
        let mut stmt = self.conn.prepare_cached(
            r#"
            select name, value
              from bench_run_info
              where start_time = ?
            "#,
        )?;
        let mut rows = stmt.query([start])?;
        while let Some(row) = rows.next()? {
            map.insert(row.get(0)?, row.get(1)?);
        }
        Ok(map)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BenchRunStart(u64);

impl ToSql for BenchRunStart {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for BenchRunStart {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        u64::column_result(value).map(BenchRunStart)
    }
}
