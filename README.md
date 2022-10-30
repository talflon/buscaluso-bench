# Buscaluso-bench

Benchmarking program for development of [Buscaluso](https://github.com/talflon/buscaluso).

Still under construction.
Currently outputs to a [SQLite](https://sqlite.org) database,
and includes a utility to analyze the results.

Goals: to be able to run different development development versions of the algorithm, with different settings, on words to search for and find.
To be able to compare results in terms of if they found the desired dictionary words, how much clock time it took, and how far down the list of possible words found it was.

## Usage

```
Usage: buscaluso-bench [OPTIONS] --config <CONFIG>

Options:
  -m, --machine <MACHINE>  Machine identifier
  -c, --config <CONFIG>    Config TOML file
  -r, --rules <RULES>      Rules file
  -d, --dict <DICT>        Dictionary file
  -b, --bench <BENCH>      Benchmark file
  -o, --out-db <OUT_DB>    Output database file, defaults to "bench.sqlite3"
  -v, --verbose...         Turn on verbose output
  -h, --help               Print help information
  -V, --version            Print version information
```

This will run all the benchmarks specified, with the specified settings.
They are not run in parallel, to avoid interference in measuring performance, so it will take a while.

`--config` is required, and is a [TOML](https://toml.io/) file with the following required settings:

```
repeat = <times>
timeout = <seconds>
```

which correspond to running each test `repeat` times, and waiting at least `timeout` seconds for results. There are also the following optional settings, which can also be specified on the command line as shown above, with the command line taking precedence:

```
verbose = <int level>
rules_file = <path>
dict_file = <path>
bench_file = <path>
```

The rules and dictionary files are required, and are passed to Buscaluso.

The machine identifier is required, and is a simple string to identify which machine it was run on.

The benchmark file is also required, and has the following format:

### Benchmark file format

The benchmark file is a UTF-8 text file, with one benchmark per line.
Comments start with a semicolon (`;`) and last until the end of the line.
This benchmark line:

```
search_word1, search_word2 = target_word_1A | target_word1B, target_word2
```

Corresponds to four benchmarks to be run:

1. Search starting from `search_word1`, looking for `target_word1A` or `target_word1B`, whichever comes first.
2. Same thing, but search starting from `search_word2`.
3. Search starting from `search_word1`, looking for `target_word2`.
4. Search starting from `search_word2`, looking for `target_word2`.

If any search words have accented letters, the benchmark is added for both the verbatim, accented search word,
and also for a second version of the word with all accent marks removed. There's no need to write both:

```
óne = ano
one = ano
```

because the first one suffices to define both benchmarks.

## `benchdb` utility

`benchdb` is a command-line utility to explore the results in a SQLite file.

```
Usage: benchdb [OPTIONS] <COMMAND>

Commands:
  list-sessions  Lists all sessions
  show           Shows a session's metadata. Doesn't show multiline values
  get            Outputs a single metadata value from a session
  stats          Shows some quick statistics of a session's results
  results        Shows statistics of all the session's results
  compare        Compares the results of two sessions
  help           Print this message or the help of the given subcommand(s)

Options:
      --db <DB>  Database file [default: bench.sqlite3]
  -h, --help     Print help information
```

When getting or comparing statistics, it combines multiple runs,
ignores the best and worst (except for errors),
and takes a "score" that combines the position in the results with the time spent.
At the moment, this is fixed at dropping 1/4 of the results (the top and bottom 1/8, round down),
and treating each result position as 1/8 of a second.
`compare` will only show individual benchmarks where there was a difference in score of at least 1/32 second.