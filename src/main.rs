use std::io::prelude::*;

use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;

use buscaluso::*;
use buscaluso_bench::*;

shadow_rs::shadow!(build);

#[derive(Parser)]
#[clap(author, version, long_version = build::CLAP_LONG_VERSION, about, long_about = None)]
struct Cli {
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

const REPEAT_BENCHES: u8 = 3;
const BENCH_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    let cli = Cli::parse();
    let start_time = Instant::now();
    let mut cfg = BuscaCfg::new();
    let mut bencher = Bencher::new();

    if cli.verbose > 0 {
        eprint!("Loading rules from {:?}...", cli.rules);
        io::stderr().flush()?;
    }
    cfg.load_rules(BufReader::new(File::open(cli.rules)?))?;
    if cli.verbose > 0 {
        eprintln!("done");
        eprint!("Loading dictionary from {:?}...", cli.dict);
        io::stderr().flush()?;
    }
    cfg.load_dictionary(BufReader::new(File::open(cli.dict)?))?;
    if cli.verbose > 0 {
        eprintln!("done");
        eprint!("Loading benchmarks from {:?}...", cli.bench);
        io::stderr().flush()?;
    }
    bencher.load_benches(BufReader::new(File::open(cli.bench)?))?;
    if cli.verbose > 0 {
        eprintln!("done");
        eprintln!(
            "Running all benchmarks {} times with a timeout of {:?} each",
            REPEAT_BENCHES, BENCH_TIMEOUT,
        );
    }
    bencher.run_benches(
        &cfg,
        &BenchRunCfg {
            repeat: REPEAT_BENCHES,
            timeout: BENCH_TIMEOUT,
            verbose: cli.verbose,
        },
    );
    if cli.verbose > 0 {
        let elapsed = start_time.elapsed();
        eprintln!("Total elapsed time: {:?}", elapsed);
    }

    for (bench_name, result) in bencher.compile_results() {
        println!("{} : {:?}", bench_name, result);
    }

    Ok(())
}
