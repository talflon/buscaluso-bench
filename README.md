# Buscaluso-bench

Benchmarking program for development of [Buscaluso](https://github.com/talflon/buscaluso).

Still under construction. Currently runs, but takes too long, and doesn't have a way of storing the results.

Goals: to be able to run different development development versions of the algorithm, with different settings, on words to search for and find.
To be able to compare results in terms of if they found the desired dictionary words, how much clock time it took, and how far down the list of possible words found it was.

## Usage

```
Usage: buscaluso-bench [OPTIONS] --config <CONFIG>

Options:
  -c, --config <CONFIG>  Config TOML file
  -r, --rules <RULES>    Rules file
  -d, --dict <DICT>      Dictionary file
  -b, --bench <BENCH>    Benchmark file
  -v, --verbose...       Turn on verbose output
  -h, --help             Print help information
  -V, --version          Print version information
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

The rules and dictionary files are required, and are passed to Buscaluso. The benchmark file is also required, and has the following format:

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
