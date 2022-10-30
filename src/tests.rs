// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use super::*;

use quickcheck::{Arbitrary, TestResult};
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

impl Arbitrary for BenchResultCompiler {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        let mut drop_fraction = f64::arbitrary(g);
        while !drop_fraction.is_finite() {
            drop_fraction = f64::arbitrary(g);
        }
        drop_fraction = drop_fraction.abs() % 1.0;
        BenchResultCompiler {
            index_equivalent: Duration::arbitrary(g),
            drop_fraction,
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
            out_db: default_out_db(),
            machine: None,
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
        out_db: default_out_db(),
        machine: None,
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

#[test]
fn test_set_bench_name() {
    let mut bench_name = String::new();
    set_bench_name(&mut bench_name, "word", &BTreeSet::from(["target"]));
    assert_eq!(bench_name, "word = target");
    set_bench_name(&mut bench_name, "x", &BTreeSet::from(["a", "b"]));
    assert_eq!(bench_name, "x = a | b");
    set_bench_name(&mut bench_name, "y", &BTreeSet::from(["a", "b", "c"]));
    assert_eq!(bench_name, "y = a | b | c");
}

#[test]
fn test_set_bench_name_canonical_order() {
    let mut bench_name1 = String::new();
    let mut bench_name2 = String::new();
    set_bench_name(
        &mut bench_name1,
        "does",
        &BTreeSet::from_iter(["this", "work"].iter()),
    );
    set_bench_name(
        &mut bench_name2,
        "does",
        &BTreeSet::from_iter(["work", "this"].iter()),
    );
    assert_eq!(bench_name1, bench_name2);
}

#[test]
fn test_get_build_info() {
    let info = get_build_info();
    for (key, value) in &info {
        assert!(!value.trim().is_empty(), "empty value for {}", key);
    }
}

#[quickcheck]
fn test_get_range(values: Vec<i8>) -> TestResult {
    if values.is_empty() {
        return TestResult::discard();
    }
    let least: i8 = values.iter().copied().min().unwrap();
    let most: i8 = values.iter().copied().max().unwrap();
    TestResult::from_bool(get_range(values.iter().copied()) == Some(least..=most))
}

#[test]
fn test_get_range_empty() {
    assert!(get_range(&[] as &[i8]).is_none());
}

#[test]
fn test_extend_range() {
    assert_eq!(extend_range(0..=5, 3), 0..=5);
    assert_eq!(extend_range(0..=5, 9), 0..=9);
    assert_eq!(extend_range(12..=15, 2), 2..=15);
}

#[quickcheck]
fn test_extend_range_contains(range: RangeInclusive<i8>, value: i8) -> TestResult {
    if range.is_empty() {
        return TestResult::discard();
    }
    TestResult::from_bool(extend_range(range, value).contains(&value))
}

#[quickcheck]
fn test_combine_ranges_contains_all(
    range1: RangeInclusive<i8>,
    range2: RangeInclusive<i8>,
) -> TestResult {
    if range1.is_empty() || range2.is_empty() {
        return TestResult::discard();
    }
    let combined = combine_ranges(&range1, &range2);
    range1.into_iter().all(|value| combined.contains(&value));
    range2.into_iter().all(|value| combined.contains(&value));
    TestResult::passed()
}

#[quickcheck]
fn test_resultcompiler_score_index(
    result: BenchResult,
    compiler: BenchResultCompiler,
) -> TestResult {
    if !result.is_found() {
        return TestResult::discard();
    }
    let result_plus_one = BenchResult {
        found_index: Ok(Some(result.found_index.as_ref().unwrap().unwrap() + 1)),
        elapsed: result.elapsed,
    };
    let orig_score = compiler.score(&result);
    let new_score = compiler.score(&result_plus_one);
    assert!(new_score > orig_score);
    assert!((new_score - orig_score - compiler.index_equivalent.as_secs_f64()).abs() < 1e-8);
    TestResult::passed()
}

#[quickcheck]
fn test_resultcompiler_score_found(result: BenchResult, compiler: BenchResultCompiler) -> bool {
    compiler.score(&result).is_infinite() != result.is_found()
}

#[quickcheck]
fn test_resultcompiler_score_elapsed(
    index: u8,
    elapsed1: Duration,
    elapsed2: Duration,
    index_equivalent: Duration,
) {
    let compiler = BenchResultCompiler::new(index_equivalent, 0.0);
    let result1 = BenchResult {
        found_index: Ok(Some(index as usize)),
        elapsed: elapsed1,
    };
    let result2 = BenchResult {
        found_index: Ok(Some(index as usize)),
        elapsed: elapsed2,
    };
    let score_diff = compiler.score(&result1) - compiler.score(&result2);
    let elapsed_diff = elapsed1.as_secs_f64() - elapsed2.as_secs_f64();
    assert!((score_diff - elapsed_diff).abs() < 1e-8);
}

#[quickcheck]
fn test_resultcompiler_collects_all_errors(
    errors: Vec<(String, Duration)>,
    mut results: Vec<BenchResult>,
    compiler: BenchResultCompiler,
) -> TestResult {
    if errors.is_empty() {
        return TestResult::discard();
    }
    results.extend(errors.iter().map(|(err, elapsed)| BenchResult {
        found_index: Err(err.clone()),
        elapsed: *elapsed,
    }));
    let compiled = compiler.compile(results);
    for (err, _) in &errors {
        assert!(compiled.errors.contains(err));
    }
    TestResult::passed()
}

#[quickcheck]
fn test_resultcompiler_index_range_no_drop(indices: Vec<Option<usize>>) -> TestResult {
    if indices.is_empty() {
        return TestResult::discard();
    }
    let compiler = BenchResultCompiler::new(Duration::ZERO, 0.0);
    let compiled = compiler.compile(indices.iter().map(|&index| BenchResult {
        found_index: Ok(index),
        elapsed: Default::default(),
    }));
    assert_eq!(
        compiled.found_index,
        get_range(indices.iter().flatten().copied())
    );
    TestResult::passed()
}

#[quickcheck]
fn test_resultcompiler_elapsed_range_no_drop(times: Vec<(Duration, bool)>) -> TestResult {
    if times.is_empty() {
        return TestResult::discard();
    }
    let compiler = BenchResultCompiler::new(Duration::ZERO, 0.0);
    let compiled = compiler.compile(times.iter().copied().map(|(elapsed, found)| BenchResult {
        found_index: Ok(if found { Some(0) } else { None }),
        elapsed,
    }));
    assert_eq!(
        compiled.elapsed,
        get_range(
            times
                .iter()
                .filter(|(_, found)| *found)
                .map(|(elapsed, _)| *elapsed)
        )
    );
    TestResult::passed()
}

#[test]
fn test_resultcompiler_drop_fraction_elapsed() {
    assert_eq!(
        BenchResultCompiler::new(Duration::ZERO, 0.5)
            .compile([
                BenchResult::success(0, Duration::from_secs(1)),
                BenchResult::success(0, Duration::from_secs(2)),
                BenchResult::success(0, Duration::from_secs(3)),
                BenchResult::success(0, Duration::from_secs(4)),
                BenchResult::success(0, Duration::from_secs(5)),
            ])
            .elapsed,
        Some(Duration::from_secs(2)..=Duration::from_secs(4))
    );
    assert_eq!(
        BenchResultCompiler::new(Duration::ZERO, 0.5)
            .compile([
                BenchResult::success(0, Duration::from_secs(10)),
                BenchResult::success(0, Duration::from_secs(11)),
                BenchResult::success(0, Duration::from_secs(12)),
                BenchResult::success(0, Duration::from_secs(13)),
            ])
            .elapsed,
        Some(Duration::from_secs(11)..=Duration::from_secs(12))
    );
    assert_eq!(
        BenchResultCompiler::new(Duration::ZERO, 0.5)
            .compile([
                BenchResult::success(0, Duration::from_secs(0)),
                BenchResult::success(0, Duration::from_secs(1)),
                BenchResult::success(0, Duration::from_secs(2)),
            ])
            .elapsed,
        Some(Duration::from_secs(0)..=Duration::from_secs(2))
    );
}

#[test]
fn test_resultcompiler_drop_fraction_found_index() {
    assert_eq!(
        BenchResultCompiler::new(Duration::from_secs(2), 0.5)
            .compile([
                BenchResult::success(1, Duration::ZERO),
                BenchResult::success(2, Duration::ZERO),
                BenchResult::success(3, Duration::ZERO),
                BenchResult::success(4, Duration::ZERO),
                BenchResult::success(5, Duration::ZERO),
            ])
            .found_index,
        Some(2..=4)
    );
    assert_eq!(
        BenchResultCompiler::new(Duration::from_secs(2), 0.5)
            .compile([
                BenchResult::success(10, Duration::ZERO),
                BenchResult::success(11, Duration::ZERO),
                BenchResult::success(12, Duration::ZERO),
                BenchResult::success(13, Duration::ZERO),
            ])
            .found_index,
        Some(11..=12)
    );
    assert_eq!(
        BenchResultCompiler::new(Duration::from_secs(2), 0.5)
            .compile([
                BenchResult::success(0, Duration::ZERO),
                BenchResult::success(1, Duration::ZERO),
                BenchResult::success(2, Duration::ZERO),
            ])
            .found_index,
        Some(0..=2)
    );
}

#[test]
fn test_resultcompiler_drop_fraction_score() {
    assert_eq!(
        BenchResultCompiler::new(Duration::from_secs(2), 0.6)
            .compile([
                BenchResult::success(1, Duration::from_secs_f64(0.1)),
                BenchResult::success(2, Duration::from_secs_f64(1.0)),
                BenchResult::success(3, Duration::from_secs_f64(0.5)),
                BenchResult::success(5, Duration::from_secs_f64(2.3)),
            ])
            .score,
        Some(Duration::from_secs_f64(5.75))
    );
}
