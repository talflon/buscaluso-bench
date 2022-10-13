// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

mod benchfile;
mod sqlite;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::io::BufRead;
use std::path::PathBuf;
use std::result::Result;
use std::time::{Duration, Instant};

use nom::Finish;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use buscaluso::BuscaCfg;

#[derive(Error, Debug)]
pub enum BenchError {
    #[error("IO error {source:?}")]
    Io {
        #[from]
        source: io::Error,
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
                                benches.get_mut(target).unwrap().push(BenchResult {
                                    elapsed,
                                    found_index: Ok(Some(word_idx)),
                                })
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

    pub fn compile_results(&self) -> Vec<(String, BenchResult)> {
        let mut results = Vec::new();
        for (start_word, benches) in &self.benches {
            for (targets, run_results) in benches {
                let mut bench_name = start_word.clone();
                bench_name.push_str(" = ");
                let targets: Vec<&str> = targets.iter().map(String::as_ref).collect();
                bench_name.push_str(&targets.join(" | "));
                results.push((bench_name, compile_run_results(run_results)));
            }
        }
        results
    }
}

fn compile_run_results(run_results: &[BenchResult]) -> BenchResult {
    run_results.iter().min().unwrap().clone()
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

fn set_unaccented(accented: &str, unaccented: &mut String) {
    unaccented.clear();
    unaccented.extend(accented.nfd().filter(|&c| !is_combining_mark(c)));
}
