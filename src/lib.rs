// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

mod benchfile;
pub mod sqlite;

shadow_rs::shadow!(build);

#[cfg(test)]
mod tests;

use std::cmp::{max, min, Ordering};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, Read};
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::time::{Duration, Instant};

use nom::Finish;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use shadow_rs::formatcp;
use thiserror::Error;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use buscaluso::BuscaCfg;

#[derive(Error, Debug)]
pub enum BenchError {
    #[error("IO error {source:?}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Parsing error on line {line_no}: {text:?}")]
    ParseErr { line_no: usize, text: String },
}

use BenchError::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchResult {
    found_index: Result<Option<usize>, String>,
    elapsed: Duration,
}

impl BenchResult {
    fn success(found_index: usize, elapsed: Duration) -> BenchResult {
        BenchResult {
            found_index: Ok(Some(found_index)),
            elapsed,
        }
    }

    fn is_found(&self) -> bool {
        matches!(self.found_index, Ok(Some(_)))
    }
}

impl PartialOrd for BenchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BenchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.found_index, &other.found_index) {
            (Ok(_), Err(_)) | (Ok(None), Ok(Some(_))) => return Ordering::Greater,
            (Err(_), Ok(_)) | (Ok(Some(_)), Ok(None)) => return Ordering::Less,
            _ => {}
        }
        (&self.found_index, self.elapsed).cmp(&(&other.found_index, other.elapsed))
    }
}

#[derive(Debug, Clone)]
pub struct BenchResultCompiler {
    index_equivalent: Duration,
    drop_fraction: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledBenchResult {
    pub score: Option<Duration>,
    pub errors: Vec<String>,
    pub found_index: Option<RangeInclusive<usize>>,
    pub elapsed: Option<RangeInclusive<Duration>>,
}

impl CompiledBenchResult {
    pub fn is_better_than(&self, other: &CompiledBenchResult) -> bool {
        match (self.score, other.score) {
            (Some(us), Some(them)) => us < them,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }

    pub fn difference(&self, other: &CompiledBenchResult) -> f64 {
        match (self.score, other.score) {
            (Some(us), Some(them)) => us.as_secs_f64() - them.as_secs_f64(),
            (Some(_), None) => f64::NEG_INFINITY,
            (None, Some(_)) => f64::INFINITY,
            (None, None) => 0.0,
        }
    }
}

pub fn extend_range<T: Ord + Copy>(range: RangeInclusive<T>, value: T) -> RangeInclusive<T> {
    if value < *range.start() {
        value..=*range.end()
    } else if value > *range.end() {
        *range.start()..=value
    } else {
        range
    }
}

pub fn combine_ranges<T: Ord + Copy>(
    range1: &RangeInclusive<T>,
    range2: &RangeInclusive<T>,
) -> RangeInclusive<T> {
    min(*range1.start(), *range2.start())..=max(*range1.end(), *range2.end())
}

pub fn get_range<T: Ord + Copy>(values: impl IntoIterator<Item = T>) -> Option<RangeInclusive<T>> {
    let mut iter = values.into_iter();
    iter.next()
        .map(|first| iter.fold(first..=first, extend_range))
}

impl BenchResultCompiler {
    pub fn new(index_equivalent: Duration, drop_fraction: f64) -> BenchResultCompiler {
        assert!(drop_fraction >= 0.0);
        assert!(drop_fraction < 1.0);
        BenchResultCompiler {
            index_equivalent,
            drop_fraction,
        }
    }

    pub fn score(&self, result: &BenchResult) -> f64 {
        match result.found_index {
            Ok(Some(i)) => {
                i as f64 * self.index_equivalent.as_secs_f64() + result.elapsed.as_secs_f64()
            }
            _ => f64::INFINITY,
        }
    }

    pub fn compile(&self, results: impl IntoIterator<Item = BenchResult>) -> CompiledBenchResult {
        let mut results: Vec<(f64, BenchResult)> =
            results.into_iter().map(|r| (self.score(&r), r)).collect();
        assert!(!results.is_empty());
        results.sort_by(|x, y| x.partial_cmp(y).unwrap()); // force sorting of floats

        let errors: Vec<String> = results
            .iter()
            .flat_map(|(_, r)| r.found_index.as_ref().err().cloned())
            .collect();

        let drop_num = (self.drop_fraction / 2.0 * results.len() as f64).floor() as usize;
        let keep_num = results.len() - 2 * drop_num;
        debug_assert!(keep_num > 0);
        let results = &results[drop_num..][..keep_num];

        let elapsed = get_range(
            results
                .iter()
                .filter(|(s, _)| s.is_finite())
                .map(|(_, r)| r.elapsed),
        );
        let found_index = get_range(
            results
                .iter()
                .filter(|(s, _)| s.is_finite())
                .map(|(_, r)| r.found_index.as_ref().unwrap().unwrap()),
        );

        let total: f64 = results.iter().map(|(s, _)| s).sum();
        let score = total / keep_num as f64;

        CompiledBenchResult {
            score: if score.is_finite() {
                Some(Duration::from_secs_f64(score))
            } else {
                None
            },
            errors,
            found_index,
            elapsed,
        }
    }
}

fn duration_serialize_seconds<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_f64(duration.as_secs_f64())
}

fn duration_deserialize_seconds<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let seconds: f64 = Deserialize::deserialize(deserializer)?;
    Ok(Duration::from_secs_f64(seconds))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchRunCfg {
    pub machine: Option<String>,
    pub repeat: u8,
    pub repeat_failed: u8,

    #[serde(
        serialize_with = "duration_serialize_seconds",
        deserialize_with = "duration_deserialize_seconds"
    )]
    pub timeout: Duration,

    #[serde(default)]
    pub verbose: u8,

    pub rules_file: Option<PathBuf>,
    pub dict_file: Option<PathBuf>,
    pub bench_file: Option<PathBuf>,

    #[serde(default = "default_out_db")]
    pub out_db: PathBuf,
}

fn default_out_db() -> PathBuf {
    "bench.sqlite3".into()
}

#[derive(Debug, Clone)]
pub struct Bencher {
    benches: BTreeMap<String, BTreeMap<BTreeSet<String>, Vec<BenchResult>>>,
}

impl Bencher {
    pub fn new() -> Bencher {
        Bencher {
            benches: BTreeMap::new(),
        }
    }

    pub fn add_bench<'a>(&mut self, start_word: &'a str, targets: &[&'a str]) {
        self.benches
            .entry(String::from(start_word))
            .or_default()
            .entry(BTreeSet::from_iter(
                targets.iter().map(|&s| String::from(s)),
            ))
            .or_default();
    }

    pub fn load_benches<R: BufRead>(&mut self, input: R) -> Result<(), BenchError> {
        let mut unaccented = String::new();
        for (line_no, line) in input.lines().enumerate() {
            match benchfile::bench_line(&line?).finish() {
                Ok((_, Some((start_words, target_list)))) => {
                    for start_word in start_words {
                        set_unaccented(start_word, &mut unaccented);
                        for targets in &target_list {
                            self.add_bench(start_word, targets);
                            if unaccented != start_word {
                                self.add_bench(&unaccented, targets);
                            }
                        }
                    }
                    Ok(())
                }
                Ok((_, None)) => Ok(()),
                Err(parse_err) => Err(ParseErr {
                    line_no: line_no + 1,
                    text: parse_err.input.to_owned(),
                }),
            }?;
        }
        Ok(())
    }

    pub fn clear_results(&mut self) {
        for bench_map in self.benches.values_mut() {
            bench_map.values_mut().for_each(Vec::clear);
        }
    }

    pub fn clear_successes(&mut self) {
        for bench_map in self.benches.values_mut() {
            for bench_vec in bench_map.values_mut() {
                let mut i = 0;
                while i < bench_vec.len() {
                    if bench_vec[i].is_found() {
                        bench_vec.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }

    pub fn run_benches(&mut self, search_cfg: &BuscaCfg, run_cfg: &BenchRunCfg) {
        let mut rng = thread_rng();
        let mut start_words: Vec<String> = self.benches.keys().cloned().collect();
        let num_to_do = start_words.len() as u32 * (run_cfg.repeat as u32);
        let mut num_complete: u32 = 0;

        if run_cfg.verbose > 1 {
            eprintln!("warmup run");
        }
        start_words.shuffle(&mut rng);
        for word in &start_words {
            self.run_benches_for_word(search_cfg, run_cfg, word);
        }
        self.clear_successes();

        if run_cfg.verbose > 1 {
            eprintln!("(0/{})", num_to_do);
        }
        for _ in 0..run_cfg.repeat {
            start_words.shuffle(&mut rng);
            for word in &start_words {
                self.run_benches_for_word(search_cfg, run_cfg, word);
                num_complete += 1;
                if run_cfg.verbose > 1 {
                    eprintln!("({}/{})", num_complete, num_to_do);
                }
            }
        }
    }

    fn run_benches_for_word(&mut self, cfg: &BuscaCfg, run_cfg: &BenchRunCfg, start_word: &str) {
        let benches = self.benches.get_mut(start_word).unwrap();
        let mut runner = BenchRunner::new();
        for (targets, results) in benches.iter() {
            if results.len() < run_cfg.repeat_failed as usize
                || results.iter().any(BenchResult::is_found)
            {
                runner.add_targets(targets);
            }
        }
        if runner.is_done() {
            if run_cfg.verbose > 1 {
                eprintln!("Skipping {}", start_word);
            }
            return;
        }

        let start_time = Instant::now();
        match cfg.search(start_word) {
            Ok(mut iter) => {
                let mut iter = iter.iter();
                let mut word_idx = 0;
                while !runner.is_done() {
                    match iter.next() {
                        Some(Some((word, _))) => {
                            let elapsed = start_time.elapsed();
                            runner.on_word_found(word, |target| {
                                benches
                                    .get_mut(target)
                                    .unwrap()
                                    .push(BenchResult::success(word_idx, elapsed))
                            });
                            word_idx += 1;
                        }
                        Some(None) => {}
                        None => break,
                    }

                    if start_time.elapsed() >= run_cfg.timeout {
                        break;
                    }
                }

                let elapsed = start_time.elapsed();
                for target in &runner.remaining_targets {
                    benches.get_mut(target).unwrap().push(BenchResult {
                        elapsed,
                        found_index: Ok(None),
                    });
                }
            }
            Err(err) => {
                let elapsed = start_time.elapsed();
                for result_vec in benches.values_mut() {
                    result_vec.push(BenchResult {
                        elapsed,
                        found_index: Err(err.to_string()),
                    });
                }
            }
        }
    }

    pub fn get_results(&self) -> Vec<(String, BenchResult)> {
        let mut results = Vec::new();
        let mut bench_name = String::new();
        for (start_word, benches) in &self.benches {
            for (targets, run_results) in benches {
                set_bench_name(&mut bench_name, start_word, targets);
                for result in run_results {
                    results.push((bench_name.clone(), result.clone()));
                }
            }
        }
        results
    }
}

impl Default for Bencher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct BenchRunner {
    remaining_targets: Vec<BTreeSet<String>>,
    all_target_words: BTreeSet<String>,
}

impl BenchRunner {
    fn new() -> Self {
        BenchRunner {
            remaining_targets: Vec::new(),
            all_target_words: BTreeSet::new(),
        }
    }

    fn add_targets(&mut self, targets: &BTreeSet<String>) {
        self.remaining_targets.push(targets.clone());
        for word in targets {
            self.all_target_words.insert(word.clone());
        }
    }

    fn is_done(&self) -> bool {
        self.remaining_targets.is_empty()
    }

    fn on_word_found(&mut self, word: &str, mut on_target_hit: impl FnMut(&BTreeSet<String>)) {
        if self.all_target_words.contains(word) {
            let mut target_idx = 0;
            while target_idx < self.remaining_targets.len() {
                if self.remaining_targets[target_idx].contains(word) {
                    let target = self.remaining_targets.swap_remove(target_idx);
                    on_target_hit(&target);
                } else {
                    target_idx += 1;
                }
            }
        }
    }
}

fn set_bench_name<S: AsRef<str>>(bench_name: &mut String, start_word: &str, targets: &BTreeSet<S>) {
    bench_name.clear();
    bench_name.push_str(start_word);
    bench_name.push_str(" = ");
    let mut target_iter = targets.iter();
    bench_name.push_str(target_iter.next().unwrap().as_ref());
    for target in target_iter {
        bench_name.push_str(" | ");
        bench_name.push_str(target.as_ref());
    }
}

fn set_unaccented(accented: &str, unaccented: &mut String) {
    unaccented.clear();
    unaccented.extend(accented.nfd().filter(|&c| !is_combining_mark(c)));
}

pub fn get_build_info() -> BTreeMap<&'static str, &'static str> {
    let mut map = BTreeMap::new();
    map.insert("version_bench", build::GIT_DESCRIBE);
    map.insert("version_buscaluso", buscaluso::build::GIT_DESCRIBE);
    map.insert("build_deps", build::CARGO_TREE);
    map.insert(
        "build_rust",
        formatcp!("{} {}", build::RUST_VERSION, build::RUST_CHANNEL),
    );
    map
}

pub fn file_sha256_hex(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = [0; 4096];
    let mut digest = Sha256::new();
    while let Some(len) = NonZeroUsize::new(file.read(&mut buffer)?) {
        digest.update(&buffer[..len.get()]);
    }
    Ok(hex::encode(digest.finalize()))
}
