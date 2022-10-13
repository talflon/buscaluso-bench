// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use super::*;

use quickcheck::Arbitrary;
use quickcheck_macros::*;

impl Arbitrary for BenchResult {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let found_index: Result<Option<u16>, String> = Result::arbitrary(g);
        BenchResult {
            found_index: found_index.map(|opt| opt.map(|n16| n16 as usize)),
            elapsed: Duration::arbitrary(g),
        }
    }
}

#[test]
fn test_runcfg_deserialize() {
    assert_eq!(
        toml::from_str(
            r#"
            repeat = 7
            repeat_failed = 2
            timeout = 8.3
            verbose = 1
            "#
        ),
        Ok(BenchRunCfg {
            repeat: 7,
            repeat_failed: 2,
            timeout: Duration::from_secs_f64(8.3),
            verbose: 1,
            rules_file: None,
            dict_file: None,
            bench_file: None,
        })
    );
}

#[test]
fn test_runcfg_deserialize_verbose_default() -> Result<(), toml::de::Error> {
    let cfg: BenchRunCfg = toml::from_str(
        r#"
        repeat = 7
        repeat_failed = 3
        timeout = 8.3
        "#,
    )?;
    assert_eq!(cfg.verbose, 0);
    Ok(())
}

#[test]
fn test_runcfg_serialize_deserialize() -> Result<(), toml::ser::Error> {
    let cfg = BenchRunCfg {
        repeat: 20,
        repeat_failed: 1,
        verbose: 5,
        timeout: Duration::from_secs_f64(2.5),
        rules_file: None,
        dict_file: None,
        bench_file: None,
    };
    assert_eq!(toml::from_str(&toml::to_string(&cfg)?), Ok(cfg));
    Ok(())
}

#[quickcheck]
fn test_bench_result_is_found_error(err: String, elapsed: Duration) -> bool {
    let result = BenchResult {
        found_index: Err(err),
        elapsed,
    };
    !result.is_found()
}

#[quickcheck]
fn test_bench_result_is_found_none(elapsed: Duration) -> bool {
    let result = BenchResult {
        found_index: Ok(None),
        elapsed,
    };
    !result.is_found()
}

#[quickcheck]
fn test_bench_result_is_found_some(index: usize, elapsed: Duration) -> bool {
    let result = BenchResult {
        found_index: Ok(Some(index)),
        elapsed,
    };
    result.is_found()
}

#[quickcheck]
fn test_bencher_clear_successes(results: Vec<BenchResult>) {
    let mut bencher = Bencher::new();
    bencher.add_bench("one", &["two", "three"]);
    let bencher_results: &mut Vec<BenchResult> = bencher
        .benches
        .get_mut("one")
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    for r in &results {
        bencher_results.push(r.clone());
    }
    bencher.clear_successes();
    let mut bencher_results: Vec<BenchResult> = bencher
        .benches
        .get("one")
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    bencher_results.sort();
    let mut expected: Vec<BenchResult> =
        results.iter().filter(|r| !r.is_found()).cloned().collect();
    expected.sort();
    assert_eq!(bencher_results, expected);
}

#[test]
fn test_bench_runner_is_done() {
    let mut runner = BenchRunner::new();
    assert!(runner.is_done());
    runner.add_targets(&BTreeSet::from(["s".to_string()]));
    assert!(!runner.is_done());
}

#[test]
fn test_bench_runner_on_word_found_empty() {
    let word = "word";
    BenchRunner::new().on_word_found(word, |target| {
        panic!("{:?} hit unexpected target {:?}", word, target)
    });
}

#[test]
fn test_bench_runner_on_word_found_different() {
    let mut runner = BenchRunner::new();
    runner.add_targets(&BTreeSet::from(["s".to_string()]));
    let word = "word";
    BenchRunner::new().on_word_found(word, |target| {
        panic!("{:?} hit unexpected target {:?}", word, target)
    });
}

#[test]
fn test_bench_runner_on_word_found() {
    let mut runner = BenchRunner::new();
    runner.add_targets(&BTreeSet::from(["word".to_string()]));
    runner.on_word_found("word", |_| {});
    assert!(runner.is_done());
}

#[test]
fn test_set_unaccented_already_unaccented() {
    let mut unaccented = String::new();
    let word = "simple";
    set_unaccented(word, &mut unaccented);
    assert_eq!(unaccented, word);
}

#[test]
fn test_set_unaccented() {
    let mut unaccented = String::new();
    set_unaccented("âéïõù", &mut unaccented);
    assert_eq!(unaccented, "aeiou");
}
