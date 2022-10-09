use std::io::prelude::*;

use std::fs;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;

use buscaluso::*;
use buscaluso_bench::*;

shadow_rs::shadow!(build);

#[derive(Parser)]
#[clap(author, version, long_version = build::CLAP_LONG_VERSION, about, long_about = None)]
struct Cli {
    /// Config TOML file
    #[arg(short, long)]
    config: PathBuf,

    /// Rules file
    #[arg(short, long)]
    rules: PathBuf,

    /// Dictionary file
    #[arg(short, long)]
    dict: PathBuf,

    /// Benchmark file
    #[arg(short, long)]
    bench: PathBuf,

    /// Turn on verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    let cli = Cli::parse();
    let start_time = Instant::now();
    let mut search_cfg = BuscaCfg::new();
    let mut run_cfg: BenchRunCfg =
        toml::from_str(&fs::read_to_string(cli.config).unwrap()).unwrap();
    if cli.verbose != 0 {
        run_cfg.verbose = cli.verbose;
    }
    let mut bencher = Bencher::new();

    if run_cfg.verbose > 0 {
        eprint!("Loading rules from {:?}...", cli.rules);
        io::stderr().flush().unwrap();
    }
    search_cfg
        .load_rules(BufReader::new(File::open(cli.rules).unwrap()))
        .unwrap();
    if run_cfg.verbose > 0 {
        eprintln!("done");
        eprint!("Loading dictionary from {:?}...", cli.dict);
        io::stderr().flush().unwrap();
    }
    search_cfg
        .load_dictionary(BufReader::new(File::open(cli.dict).unwrap()))
        .unwrap();
    if run_cfg.verbose > 0 {
        eprintln!("done");
        eprint!("Loading benchmarks from {:?}...", cli.bench);
        io::stderr().flush().unwrap();
    }
    bencher
        .load_benches(BufReader::new(File::open(cli.bench).unwrap()))
        .unwrap();
    if run_cfg.verbose > 0 {
        eprintln!("done");
        eprintln!(
            "Running all benchmarks {} times with a timeout of {:?} each",
            run_cfg.repeat, run_cfg.timeout,
        );
    }
    bencher.run_benches(&search_cfg, &run_cfg);
    if run_cfg.verbose > 0 {
        let elapsed = start_time.elapsed();
        eprintln!("Total elapsed time: {:?}", elapsed);
    }

    for (bench_name, result) in bencher.compile_results() {
        println!("{} : {:?}", bench_name, result);
    }
}
