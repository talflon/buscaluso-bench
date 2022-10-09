use std::io::prelude::*;

use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

use clap::{CommandFactory, Parser};

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
    rules: Option<PathBuf>,

    /// Dictionary file
    #[arg(short, long)]
    dict: Option<PathBuf>,

    /// Benchmark file
    #[arg(short, long)]
    bench: Option<PathBuf>,

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
    BufReader::new(File::open(path).unwrap())
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
    copy_required_setting_from_cli(&mut run_cfg.rules_file, &cli.rules, "Missing rules file");
    copy_required_setting_from_cli(&mut run_cfg.dict_file, &cli.dict, "Missing dict file");
    copy_required_setting_from_cli(&mut run_cfg.bench_file, &cli.bench, "Missing benches file");
    let mut bencher = Bencher::new();

    search_cfg
        .load_rules(setting_file_reader(&run_cfg.rules_file, run_cfg.verbose))
        .unwrap();
    search_cfg
        .load_dictionary(setting_file_reader(&run_cfg.dict_file, run_cfg.verbose))
        .unwrap();
    bencher
        .load_benches(setting_file_reader(&run_cfg.bench_file, run_cfg.verbose))
        .unwrap();
    if run_cfg.verbose > 0 {
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
