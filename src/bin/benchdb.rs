// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use std::fmt::Display;
use std::iter::zip;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use buscaluso_bench::sqlite::{BenchDb, BenchSessionId};
use buscaluso_bench::{combine_ranges, extend_range, BenchResultCompiler};

use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags};
use time::macros::format_description;
use time::OffsetDateTime;

#[derive(Parser)]
struct Cli {
    /// Database file
    #[arg(long, default_value = "bench.sqlite3")]
    db: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Lists all sessions.
    ListSessions,

    /// Shows a session's metadata.
    /// Doesn't show multiline values.
    Show {
        /// Session ID
        session: BenchSessionId,
    },

    /// Outputs a single metadata value from a session.
    Get {
        /// Session ID
        session: BenchSessionId,
        info_key: String,
    },

    /// Shows some quick statistics of a session's results.
    Stats {
        /// Session ID
        session: BenchSessionId,
    },

    /// Shows statistics of all the session's results.
    Results {
        /// Session ID
        session: BenchSessionId,
    },

    /// Compares the results of two sessions.
    Compare {
        /// Session ID
        session_a: BenchSessionId,
        /// Session ID
        session_b: BenchSessionId,
    },
}

const LIST_SESSIONS_EXTRA_COLUMNS: &[&str] = &["version_buscaluso", "machine", "search_rules_hash"];

const COMPARE_INDEX_EQUIVALENT: f64 = 1.0 / 8.0;
const COMPARE_DROP_FRACTION: f64 = 1.0 / 4.0;
const COMPARE_MIN_DIFFERENCE: f64 = 1.0 / 32.0;

fn format_datetime(when: SystemTime) -> String {
    OffsetDateTime::from(when)
        .format(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second]"
        ))
        .expect("Couldn't format SystemTime into Y-M-D H:M:S")
}

fn fmt_duration(duration: &Option<Duration>) -> String {
    match duration {
        Some(value) => format!("{:7.4}", value.as_secs_f64()),
        None => "--".into(),
    }
}

fn fmt_duration_range(duration_range: &Option<RangeInclusive<Duration>>) -> String {
    match duration_range {
        Some(range) => {
            let start = range.start();
            let end = range.end();
            if start == end {
                format!("{:7.4}", start.as_secs_f64())
            } else {
                format!("{:7.4} .. {:7.4}", start.as_secs_f64(), end.as_secs_f64())
            }
        }
        None => "--".into(),
    }
}

fn fmt_range<T: Display + Eq>(range: &Option<RangeInclusive<T>>) -> String {
    match range {
        Some(range) => {
            let start = range.start();
            let end = range.end();
            if start == end {
                format!("{}", start)
            } else {
                format!("{} .. {}", start, end)
            }
        }
        None => "--".into(),
    }
}

impl Command {
    fn run(&self, db: &mut BenchDb) -> rusqlite::Result<()> {
        match *self {
            Command::ListSessions => {
                let sessions: rusqlite::Result<Vec<(BenchSessionId, usize)>> = db
                    .conn
                    .prepare(
                        r#"
                        select session_id, count(*)
                            from (select distinct session_id, bench
                                  from bench_run)
                            group by session_id
                            order by session_id desc
                        "#,
                    )?
                    .query_map((), |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect();
                let sessions = sessions?;
                let mut table = AlignedTable::new_cloned(
                    ["SESSION ID", "WHEN", "NUM BENCHES"]
                        .iter()
                        .chain(LIST_SESSIONS_EXTRA_COLUMNS.iter()),
                    " | ",
                );
                for (session_id, num_benches) in sessions {
                    let mut row = vec![
                        session_id.to_string(),
                        format_datetime(session_id.start_time()),
                        num_benches.to_string(),
                    ];
                    for key in LIST_SESSIONS_EXTRA_COLUMNS {
                        row.push(db.get_info(session_id, key)?);
                    }
                    table.add_row(row);
                }

                println!("{}", table);
            }

            Command::Show { session } => {
                let mut table = AlignedTable::new_cloned(["KEY", "VALUE"], " | ");
                let info = db.get_all_info(session)?;
                if info.is_empty() {
                    println!("Session not found");
                } else {
                    for (key, value) in info {
                        table.add_row(vec![
                            key,
                            if value.contains('\n') {
                                "<...>".to_string()
                            } else {
                                value
                            },
                        ]);
                    }
                    println!("{}", table);
                }
            }

            Command::Get {
                session,
                ref info_key,
            } => {
                let value = db.get_info(session, info_key)?;
                print!("{}", value);
                if !value.is_empty() && !value.ends_with('\n') {
                    println!();
                }
            }

            Command::Results { session } => {
                let benches: rusqlite::Result<Vec<String>> = db
                    .conn
                    .prepare(
                        r#"
                        select distinct bench
                            from bench_run
                            where session_id = ?
                        "#,
                    )?
                    .query_map([session], |row| row.get(0))?
                    .collect();
                let benches = benches?;
                if benches.is_empty() {
                    println!("Session not found");
                } else {
                    let compiler = BenchResultCompiler::new(
                        Duration::from_secs_f64(COMPARE_INDEX_EQUIVALENT),
                        COMPARE_DROP_FRACTION,
                    );
                    let mut table =
                        AlignedTable::new_cloned(["BENCH", "SCORE", "INDEX", "TIME (sec)"], " | ");
                    for bench in benches {
                        let compiled = compiler.compile(db.get_results(session, &bench)?);
                        table.add_row(vec![
                            bench,
                            fmt_duration(&compiled.score),
                            fmt_range(&compiled.found_index),
                            fmt_duration_range(&compiled.elapsed),
                        ]);
                    }
                    println!("{}", table);
                }
            }

            Command::Stats { session } => {
                let benches: rusqlite::Result<Vec<String>> = db
                    .conn
                    .prepare(
                        r#"
                        select distinct bench
                            from bench_run
                            where session_id = ?
                        "#,
                    )?
                    .query_map([session], |row| row.get(0))?
                    .collect();
                let benches = benches?;
                if benches.is_empty() {
                    println!("Session not found");
                } else {
                    let compiler = BenchResultCompiler::new(
                        Duration::from_secs_f64(COMPARE_INDEX_EQUIVALENT),
                        COMPARE_DROP_FRACTION,
                    );
                    let mut num_found = 0;
                    let mut total_score = 0.0;
                    let mut score_range: Option<RangeInclusive<Duration>> = None;
                    let mut elapsed_range: Option<RangeInclusive<Duration>> = None;
                    for bench in &benches {
                        let compiled = compiler.compile(db.get_results(session, bench)?);
                        if let (Some(score), Some(_found_index), Some(elapsed)) =
                            (compiled.score, compiled.found_index, compiled.elapsed)
                        {
                            total_score += score.as_secs_f64();
                            score_range =
                                Some(score_range.map_or_else(
                                    || score..=score,
                                    |range| extend_range(range, score),
                                ));
                            elapsed_range = Some(elapsed_range.map_or_else(
                                || elapsed.clone(),
                                |range| combine_ranges(&range, &elapsed),
                            ));
                            num_found += 1;
                        }
                    }
                    println!(
                        "Found {} / {} ({:.1}%)",
                        num_found,
                        benches.len(),
                        num_found as f64 / benches.len() as f64 * 100.0
                    );
                    if num_found > 0 {
                        let avg_score = Duration::from_secs_f64(total_score / num_found as f64);
                        println!("Average score: {} sec", fmt_duration(&Some(avg_score)));
                        println!("Score range: {}", fmt_duration_range(&score_range));
                        println!("Seconds to find: {}", fmt_duration_range(&elapsed_range));
                    }
                }
            }

            Command::Compare {
                session_a,
                session_b,
            } => {
                let benches: rusqlite::Result<Vec<String>> = db
                    .conn
                    .prepare(
                        r#"
                        select distinct r_a.bench
                            from bench_run as r_a,
                                 bench_run as r_b
                            where r_a.bench = r_b.bench
                              and r_a.session_id = ?
                              and r_b.session_id = ?
                            order by 1
                        "#,
                    )?
                    .query_map((session_a, session_b), |row| row.get(0))?
                    .collect();
                let benches = benches?;
                if benches.is_empty() {
                    println!("Session not found, or no benches in common");
                } else {
                    let mut tables: [AlignedTable; 2] = [(); 2].map(|_| {
                        AlignedTable::new_cloned(
                            [
                                "BENCH",
                                "A: SCORE",
                                "B: SCORE",
                                "A: INDEX",
                                "A: TIME (sec)",
                                "B: INDEX",
                                "B: TIME (sec)",
                            ],
                            " | ",
                        )
                    });
                    let compiler = BenchResultCompiler::new(
                        Duration::from_secs_f64(COMPARE_INDEX_EQUIVALENT),
                        COMPARE_DROP_FRACTION,
                    );
                    let min_difference = COMPARE_MIN_DIFFERENCE;
                    let mut total_difference = 0.0;
                    let mut wins_a = 0;
                    let mut wins_b = 0;
                    for bench in benches {
                        let result_a = compiler.compile(db.get_results(session_a, &bench)?);
                        let result_b = compiler.compile(db.get_results(session_b, &bench)?);
                        let difference = result_a.difference(&result_b);
                        if difference.is_finite() {
                            total_difference += difference;
                        } else if difference < 0.0 {
                            wins_a += 1;
                        } else {
                            wins_b += 1;
                        }
                        if difference.abs() >= min_difference {
                            tables[!result_a.is_better_than(&result_b) as usize].add_row(vec![
                                bench,
                                fmt_duration(&result_a.score),
                                fmt_duration(&result_b.score),
                                fmt_range(&result_a.found_index),
                                fmt_duration_range(&result_a.elapsed),
                                fmt_range(&result_b.found_index),
                                fmt_duration_range(&result_b.elapsed),
                            ]);
                        }
                    }
                    if wins_a > 0 {
                        println!("A found {} that B didn't", wins_a);
                    }
                    if wins_b > 0 {
                        println!("B found {} that A didn't", wins_b);
                    }
                    print!("Total minor score differences: ");
                    if total_difference > 0.0 {
                        println!(
                            "B better by {} sec",
                            fmt_duration(&Some(Duration::from_secs_f64(total_difference)))
                        );
                    } else if total_difference < 0.0 {
                        println!(
                            "A better by {} sec",
                            fmt_duration(&Some(Duration::from_secs_f64(-total_difference)))
                        );
                    } else {
                        println!("none");
                    }
                    for (name, table) in zip(["A", "B"], tables) {
                        if !table.is_empty() {
                            println!("\nBetter in {}:\n{}", name, table);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct AlignedTable {
    rows: Vec<Vec<String>>,
    separator: String,
    widths: Vec<usize>,
}

impl AlignedTable {
    fn new(header: Vec<String>, separator: String) -> AlignedTable {
        assert!(!header.is_empty());
        let widths = header.iter().map(String::len).collect();
        AlignedTable {
            rows: vec![header],
            separator,
            widths,
        }
    }

    fn new_cloned(
        header: impl IntoIterator<Item = impl AsRef<str>>,
        separator: impl AsRef<str>,
    ) -> AlignedTable {
        Self::new(
            header.into_iter().map(|s| s.as_ref().to_string()).collect(),
            separator.as_ref().to_string(),
        )
    }

    fn get_header(&self) -> &[String] {
        &self.rows[0]
    }

    fn get_num_cols(&self) -> usize {
        self.get_header().len()
    }

    fn add_row(&mut self, row: Vec<String>) {
        assert!(row.len() == self.get_num_cols());
        for (i, len) in row.iter().map(String::len).enumerate() {
            if len > self.widths[i] {
                self.widths[i] = len;
            }
        }
        self.rows.push(row);
    }

    fn is_empty(&self) -> bool {
        self.rows.len() <= 1
    }
}

impl Display for AlignedTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fmt_row = |f: &mut std::fmt::Formatter<'_>, row: &[String]| {
            write!(f, "{:1$}", row[0], self.widths[0])?;
            for (value, width) in zip(row, &self.widths).skip(1) {
                write!(f, "{}{:2$}", self.separator, value, width)?;
            }
            Ok(()) as std::fmt::Result
        };

        fmt_row(f, &self.rows[0])?;
        for row in &self.rows[1..] {
            writeln!(f)?;
            fmt_row(f, row)?;
        }
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    let mut db = BenchDb::new(
        Connection::open_with_flags(&cli.db, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("Error opening db file"),
    )
    .expect("Error initializing db");
    cli.command.run(&mut db).expect("Error running command");
}
