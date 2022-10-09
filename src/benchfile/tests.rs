// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use super::*;

#[test]
fn test_simple() {
    assert_eq!(
        bench_line("bulacha = bolacha"),
        Ok(("", Some((vec!["bulacha"], vec![vec!["bolacha"]]))))
    );
}

#[test]
fn test_unicode() {
    assert_eq!(
        bench_line("assõ = ação"),
        Ok(("", Some((vec!["assõ"], vec![vec!["ação"]]))))
    );
}

#[test]
fn test_multiple_starting_words() {
    assert_eq!(
        bench_line("abc, def , ghi = xyz"),
        Ok(("", Some((vec!["abc", "def", "ghi"], vec![vec!["xyz"]]))))
    );
}

#[test]
fn test_target_options() {
    assert_eq!(
        bench_line("start = one | two"),
        Ok(("", Some((vec!["start"], vec![vec!["one", "two"]]))))
    );
}

#[test]
fn test_multiple_targets() {
    assert_eq!(
        bench_line("start = one, two"),
        Ok(("", Some((vec!["start"], vec![vec!["one"], vec!["two"]]))))
    );
}

#[test]
fn test_multiple_targets_with_options() {
    assert_eq!(
        bench_line("start = one, two | three"),
        Ok((
            "",
            Some((vec!["start"], vec![vec!["one"], vec!["two", "three"]]))
        ))
    );
}

#[test]
fn test_blank_lines() {
    for line in ["", " ", "   "] {
        assert_eq!(bench_line(line), Ok(("", None)));
    }
}

#[test]
fn test_comment_line() {
    for line in [";", ";comment", " ; yep  "] {
        assert_eq!(bench_line(line), Ok(("", None)));
    }
}

#[test]
fn test_comment_after_content() {
    assert_eq!(
        bench_line("start = one, two ;, three | four"),
        Ok(("", Some((vec!["start"], vec![vec!["one"], vec!["two"]]))))
    );
}
