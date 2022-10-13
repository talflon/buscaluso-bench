// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use quickcheck::QuickCheck;
use quickcheck_macros::quickcheck;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rusqlite::Connection;

use super::{BenchDb, BenchResult};

#[test]
fn test_db_new_idempotent() -> rusqlite::Result<()> {
    let conn = Connection::open_in_memory()?;
    let db1 = BenchDb::new(conn)?;
    let db2 = BenchDb::new(db1.conn)?;
    db2.conn.close().unwrap();
    Ok(())
}

#[test]
fn test_new_start_different() -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let start1 = db.new_start()?;
    db.set_info(start1, "x", "y")?;
    let start2 = db.new_start()?;
    assert_ne!(start2, start1);
    db.set_info(start2, "one", "two")?;
    assert_ne!(db.new_start()?, start2);
    Ok(())
}

#[quickcheck]
fn test_set_info(name: String, value: String) -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let start = db.new_start()?;
    db.set_info(start, &name, &value)?;
    assert_eq!(db.get_info(start)?.get(&name), Some(&value));
    Ok(())
}

#[quickcheck]
fn test_get_info(values: BTreeMap<String, String>) -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let start = db.new_start()?;
    for (name, value) in &values {
        db.set_info(start, name, value)?;
    }
    assert_eq!(db.get_info(start)?, values);
    Ok(())
}

#[test]
fn test_add_get_results() {
    fn add_get_results(bench_results: BTreeMap<String, Vec<BenchResult>>) -> rusqlite::Result<()> {
        let mut rng = thread_rng();
        let mut db = BenchDb::new(Connection::open_in_memory()?)?;
        let start = db.new_start()?;
        db.set_info(start, "save", "this")?;
        let mut all_results: Vec<(&str, BenchResult)> = Vec::new();
        for (bench, results) in &bench_results {
            for result in results {
                all_results.push((bench, result.clone()));
            }
        }
        all_results.shuffle(&mut rng);
        for (bench, result) in all_results {
            db.add_result(start, bench, result)?;
        }
        for (bench, mut expected) in bench_results {
            let mut from_db = db.get_results(start, &bench)?;
            expected.sort();
            from_db.sort();
            assert_eq!(from_db, expected);
        }
        Ok(())
    }
    QuickCheck::new()
        .gen(quickcheck::Gen::new(8))
        .quickcheck(add_get_results as fn(_) -> rusqlite::Result<()>);
}
