// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use quickcheck::QuickCheck;
use quickcheck_macros::quickcheck;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rusqlite::Connection;

use super::{BenchDb, BenchResult, BenchSessionId};

#[test]
fn test_db_new_idempotent() -> rusqlite::Result<()> {
    let conn = Connection::open_in_memory()?;
    let db1 = BenchDb::new(conn)?;
    let db2 = BenchDb::new(db1.conn)?;
    db2.conn.close().unwrap();
    Ok(())
}

#[test]
fn test_new_session_id_different() -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let sid1 = db.new_session_id()?;
    db.set_info(sid1, "x", "y")?;
    let sid2 = db.new_session_id()?;
    assert_ne!(sid2, sid1);
    db.set_info(sid2, "one", "two")?;
    assert_ne!(db.new_session_id()?, sid2);
    Ok(())
}

#[quickcheck]
fn test_set_and_get_info(name: String, value: String) -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let sid = db.new_session_id()?;
    assert_eq!(db.get_info(sid, &name)?, "");
    db.set_info(sid, &name, &value)?;
    assert_eq!(db.get_info(sid, &name)?, value);
    Ok(())
}

#[quickcheck]
fn test_get_all_info(values: BTreeMap<String, String>) -> rusqlite::Result<()> {
    let mut db = BenchDb::new(Connection::open_in_memory()?)?;
    let sid = db.new_session_id()?;
    for (name, value) in &values {
        db.set_info(sid, name, value)?;
    }
    assert_eq!(db.get_all_info(sid)?, values);
    Ok(())
}

#[test]
fn test_add_get_results() {
    fn add_get_results(bench_results: BTreeMap<String, Vec<BenchResult>>) -> rusqlite::Result<()> {
        let mut rng = thread_rng();
        let mut db = BenchDb::new(Connection::open_in_memory()?)?;
        let sid = db.new_session_id()?;
        let mut all_results: Vec<(&str, BenchResult)> = Vec::new();
        for (bench, results) in &bench_results {
            for result in results {
                all_results.push((bench, result.clone()));
            }
        }
        all_results.shuffle(&mut rng);
        for (bench, result) in all_results {
            db.add_result(sid, bench, result)?;
        }
        for (bench, mut expected) in bench_results {
            let mut from_db = db.get_results(sid, &bench)?;
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

#[quickcheck]
fn test_session_id_display_fromstr(id: u64) {
    let session_id = BenchSessionId(id);
    assert_eq!(format!("{}", session_id).parse(), Ok(session_id));
}
