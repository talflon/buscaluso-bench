// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use std::io::prelude::*;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

use clap::{CommandFactory, Parser};
use rusqlite::Connection;

use buscaluso::BuscaCfg;

use buscaluso_bench::build;
use buscaluso_bench::file_sha256_hex;
use buscaluso_bench::sqlite::{BenchDb, BenchSessionId};
use buscaluso_bench::{get_build_info, BenchRunCfg, Bencher};

#[derive(Parser)]
#[clap(author, version = build::GIT_DESCRIBE, long_version = build::CLAP_LONG_VERSION, about, long_about = None)]
struct Cli {
    /// Machine identifier
    #[arg(short, long)]
    machine: Option<String>,

    /// Config TOML file
    #[arg(short, long)]
    config: PathBuf,

    /// Rules file
    #[arg(short, long)]
    rules: Option<PathBuf>,

    /// Dictionary file
    #[arg(short, long)]
    dict: Option<PathBuf>,

    /// Benchmark file
    #[arg(short, long)]
    bench: Option<PathBuf>,

    /// Output database file, defaults to "bench.sqlite3"
    #[arg(short, long)]
    out_db: Option<PathBuf>,

    /// Turn on verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn copy_required_setting_from_cli<T: Clone>(
    cfg_setting: &mut Option<T>,
    cli_setting: &Option<T>,
    error_msg: &str,
) {
    if cli_setting.is_some() {
        *cfg_setting = cli_setting.clone();
    } else if cfg_setting.is_none() {
        Cli::command()
            .error(clap::error::ErrorKind::MissingRequiredArgument, error_msg)
            .exit();
    }
}

fn setting_file_reader(setting: &Option<PathBuf>, verbose: u8) -> impl BufRead {
    let path = setting.as_ref().unwrap();
    if verbose > 0 {
        eprintln!("Loading {:?}", path);
    }
    BufReader::new(File::open(path).expect("Error opening file"))
}

fn main() {
    let cli = Cli::parse();
    let start_time = Instant::now();
    let mut search_cfg = BuscaCfg::new();
    let mut run_cfg: BenchRunCfg =
        toml::from_str(&fs::read_to_string(cli.config).expect("Error reading config file"))
            .expect("Error loading config");
    if cli.verbose != 0 {
        run_cfg.verbose = cli.verbose;
    }
    copy_required_setting_from_cli(
        &mut run_cfg.machine,
        &cli.machine,
        "Missing machine identifier",
    );
    copy_required_setting_from_cli(&mut run_cfg.rules_file, &cli.rules, "Missing rules file");
    copy_required_setting_from_cli(&mut run_cfg.dict_file, &cli.dict, "Missing dict file");
    copy_required_setting_from_cli(&mut run_cfg.bench_file, &cli.bench, "Missing benches file");
    if let Some(out_db) = cli.out_db {
        run_cfg.out_db = out_db;
    }

    let mut db = BenchDb::new(Connection::open(&run_cfg.out_db).expect("Error opening db file"))
        .expect("Error initializing db");
    let mut bencher = Bencher::new();

    search_cfg
        .load_rules(setting_file_reader(&run_cfg.rules_file, run_cfg.verbose))
        .expect("Error loading rules file");
    search_cfg
        .load_dictionary(setting_file_reader(&run_cfg.dict_file, run_cfg.verbose))
        .expect("Error loading dictionary");
    bencher
        .load_benches(setting_file_reader(&run_cfg.bench_file, run_cfg.verbose))
        .expect("Error loading bench file");

    if run_cfg.verbose > 0 {
        eprintln!("Storing session info into db");
    }
    let session_id = db.new_session_id().expect("Error getting session id");
    set_session_info(&mut db, session_id, &run_cfg).expect("Error adding session info to db");

    if run_cfg.verbose > 0 {
        eprintln!(
            "Running all benchmarks {} times with a timeout of {:?} each",
            run_cfg.repeat, run_cfg.timeout,
        );
    }
    bencher.run_benches(&search_cfg, &run_cfg);

    if run_cfg.verbose > 0 {
        eprintln!("Writing results to database");
    }
    for (bench, result) in bencher.get_results() {
        db.add_result(session_id, &bench, result)
            .expect("Error adding result to db");
    }

    if run_cfg.verbose > 0 {
        let elapsed = start_time.elapsed();
        eprintln!("Total elapsed time: {:?}", elapsed);
    }
}

fn set_session_info(
    db: &mut BenchDb,
    session_id: BenchSessionId,
    run_cfg: &BenchRunCfg,
) -> rusqlite::Result<()> {
    db.set_info(session_id, "machine", run_cfg.machine.as_ref().unwrap())?;
    for (key, value) in get_build_info() {
        db.set_info(session_id, key, value)?;
    }
    db.set_info(
        session_id,
        "search_rules",
        &std::fs::read_to_string(run_cfg.rules_file.as_ref().unwrap())
            .expect("Error reading rules file"),
    )?;
    db.set_info(
        session_id,
        "search_rules_hash",
        &file_sha256_hex(run_cfg.rules_file.as_ref().unwrap()).expect("Error hashing rules file"),
    )?;
    db.set_info(
        session_id,
        "search_dict_hash",
        &file_sha256_hex(run_cfg.dict_file.as_ref().unwrap()).expect("Error hashing dict file"),
    )?;
    db.set_info(
        session_id,
        "bench_config",
        &toml::to_string(&run_cfg).expect("Error serializing run config"),
    )?;
    Ok(())
}
