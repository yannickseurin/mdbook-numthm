# mdbook-numthm

An [mdBook](https://github.com/rust-lang/mdBook) preprocessor for automatically numbering theorems, lemmas, etc.

If you're used to writing maths with LaTeX, using mdbook might be frustrating if you plan to have a lot of theorems, lemmas, definitions, etc. that you'd like to automatically number and later link to. This preprocessor kind of provides what the [amsthm](https://www.ctan.org/pkg/amsthm) package does for LaTeX.

You can see it in action [here](https://github.com/yannickseurin/crypto-book).

## Installation

Assuming you have mdBook and [mdbook-katex](https://github.com/lzanini/mdbook-katex) installed, install the crate with

```console
$ cargo install --git https://github.com/yannickseurin/mdbook-numthm
```

Then add it as a preprocessor to your `book.toml`:

```toml
[preprocessor.numthm]
```

## Usage

An environment consists of a key (an arbitrary string), a name (such as "Theorem", "Lemma", etc.), and some emphasis to be applied to the header.

It will replace all occurrences of

```text
{{key}}{label}[title]
```

into an anchor identified by `label` followed by a header consisting of the name of the environment, an automatically generated number, and the `title` in parentheses.

Fields `label` and `title` are optional.
If no label is provided, then no anchor will be created, and if no title is provided, then no title will be displayed in the header.

For example, for theorems the key is `thm`, the name is `Theorem`, and the emphasis of the header is bold.
Hence, this:

```text
{{thm}}{thm:central_limit}[Central Limit Theorem]
```

will become (assuming this is the first occurrence of the key `thm`)

```text
<a name="thm:central_limit"></a>
**Theorem 1 (Central Limit Theorem).**
```

All environments that received a label can be referred to by creating a link using

```text
{{ref: label}}
```

It will be replaced by a markdown link

```text
[Theorem 1](path/to/file.md#label)
```

If the environment had a title, it can be used in place of "Theorem 1" by using

```text
{{tref: label}}
```

which will be replaced by

```text
[Central Limit Theorem](path/to/file.md#label)
```

## Configuration

For now, environments are fixed by defaults to:

- theorem: key `thm`, name `Theorem`, bold emphasis
- lemma: key `lem`, name `Lemma`, bold emphasis
- proposition: key `prop`, name `Proposition`, bold emphasis
- definition: key `def`, name `Definition`, bold emphasis
- remark: key `rem`, name `Remark`, italic emphasis.

Each environment is numbered independently.
For example,

```text
{{thm}}
{{lem}}
{{lem}}
{{thm}}
{{lem}}
```

will become

```text
**Theorem 1.**
**Lemma 1.**
**Lemma 2.**
**Theorem 2.**
**Lemma 3.**
```

There is a single configurable option

```toml
[preprocessor.numthm]
prefix : bool
```

If `prefix` is set to true, the environment numbers will be prefixed by the section number.
For example, in Chapter 1.2, theorems would get numbered 1.2.1, 1.2.2, etc.

## TODO

- allow user-provided environments through configuration
- allow common numbering of some subsets of environments (e.g., theorems and lemmas get a common counter and definitions get an independent one).
