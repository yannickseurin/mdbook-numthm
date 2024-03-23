# mdbook-numthm

[![Crates.io](https://img.shields.io/crates/v/mdbook-numthm)](https://crates.io/crates/mdbook-numthm)
[![GitHub License](https://img.shields.io/github/license/yannickseurin/mdbook-numthm)](https://github.com/yannickseurin/mdbook-numthm/blob/main/LICENSE)

An [mdBook](https://github.com/rust-lang/mdBook) preprocessor to automatically number theorems, lemmas, etc.

If you're used to writing maths with LaTeX, using mdbook might be frustrating if you plan to have a lot of theorems, lemmas, definitions, etc. that you'd like to automatically number and later link to. This preprocessor kind of provides what the [amsthm](https://www.ctan.org/pkg/amsthm) package does for LaTeX.

You can see it in action [here](https://github.com/yannickseurin/crypto-book).

## Installation

Assuming you have mdBook and [mdbook-katex](https://github.com/lzanini/mdbook-katex) installed, install the crate with

```console
$ cargo install mdbook-numthm
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
If a label already exists, it will ignore it and emit a warning.

For example, for the "theorem" environment, the key is `thm`, the name is `Theorem`, and the emphasis of the header is bold.
Hence, this:

```text
{{thm}}{thm:central_limit}[Central Limit Theorem]
```

will become (assuming this is the first occurrence of the key `thm`)

```text
<a name="thm:central_limit"></a>
**Theorem 1 (Central Limit Theorem).**
```

and will be rendered as

> **Theorem 1 (Central Limit Theorem).**

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

If the label does not exist, it will replace the ref with **[??]** and emit a warning.

## Builtin Environments

Five builtin environments are provided:

- theorem: key `thm`, name `Theorem`, bold emphasis
- lemma: key `lem`, name `Lemma`, bold emphasis
- proposition: key `prop`, name `Proposition`, bold emphasis
- definition: key `def`, name `Definition`, bold emphasis
- remark: key `rem`, name `Remark`, italic emphasis.

## Numbering

Each environment is numbered independently.
For example,

```text
{{thm}}
{{lem}}
{{lem}}
{{thm}}
{{lem}}
```

will yield

> **Theorem 1.**  
> **Lemma 1.**  
> **Lemma 2.**  
> **Theorem 2.**  
> **Lemma 3.**

Moreover, the counter for each environment is reset at the beginning of each (sub)chapter.

## Custom Environments

It is possible to define new environments through the `custom_environments` key of the toml.
Each new environment is specified by an array `[env_key, env_name, env_emph]`, where `env_key`, `env_name`, and `env_emph` are three strings specifying the environment key, the environment name, and the environment emphasis (more specifically, the string that will be added before and after the environment heahder, e.g. `**` for bold), as defined above.
The value of the `custom_environments` must be an array of such environment-defining arrays.

Consider for example the following configuration:

```toml
[preprocessor.numthm]
custom_environments = [
  ["conj", "Conjecture", "*"],
  ["ax", "Axiom", "**"]
]
```

It defines two new environments:

- a "conjecture" environment with key `conj`, name "Conjecture", and italic emphasis,
- an "axiom" environment with key `ax`, name "Axiom", and bold emphasis 

## Configuration

There is a single configurable option

```toml
[preprocessor.numthm]
prefix = bool
```

If `prefix` is set to true, the environment numbers will be prefixed by the section number.
For example, in Chapter 1.2, theorems will get numbered 1.2.1, 1.2.2, etc.

## Interaction with other Preprocessors

If you're also using the [mdbook-footnote] preprocessor, you must ensure that it is run *after* mdbook-numthm:

```toml
[preprocessor.footnote]
after = ["numthm"]
```

## TODO

- allow common numbering of some subsets of environments (e.g., theorems and lemmas get a common counter and definitions get an independent one).
