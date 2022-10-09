// Copyright © 2022 Daniel Getz
// SPDX-License-Identifier: MIT

use nom::bytes::complete::take_while1;
use nom::character::complete::{char, space0};
use nom::combinator::{eof, opt};
use nom::multi::separated_list1;
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::IResult;

#[cfg(test)]
mod tests;

type IRes<'a, T> = IResult<&'a str, T>;

type StartWords<'a> = Vec<&'a str>;

type Target<'a> = Vec<&'a str>;

type Targets<'a> = Vec<Target<'a>>;

fn word(input: &str) -> IRes<&str> {
    take_while1(|c: char| c.is_alphanumeric())(input)
}

fn start_words(input: &str) -> IRes<StartWords> {
    separated_list1(delimited(space0, char(','), space0), word)(input)
}

fn target(input: &str) -> IRes<Target> {
    separated_list1(delimited(space0, char('|'), space0), word)(input)
}

fn targets(input: &str) -> IRes<Targets> {
    separated_list1(delimited(space0, char(','), space0), target)(input)
}

fn bench(input: &str) -> IRes<(StartWords, Targets)> {
    separated_pair(start_words, delimited(space0, char('='), space0), targets)(input)
}

fn remainder(input: &str) -> IRes<&str> {
    Ok(("", input))
}

fn comment(input: &str) -> IRes<&str> {
    preceded(char(';'), remainder)(input)
}

pub fn bench_line(input: &str) -> IRes<Option<(StartWords, Targets)>> {
    terminated(
        delimited(space0, opt(bench), preceded(space0, opt(comment))),
        eof,
    )(input)
}
