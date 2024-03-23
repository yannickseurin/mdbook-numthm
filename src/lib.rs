//! An [mdBook](https://github.com/rust-lang/mdBook) preprocessor for automatically numbering theorems, lemmas, etc.

use log::warn;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pathdiff::diff_paths;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The preprocessor name.
const NAME: &str = "numthm";

/// An environment handled by the preprocessor.
struct Env {
    /// The key to match to detect the environment, e.g. "thm".
    key: String,
    /// The name to display in the header, e.g. "Theorem".
    name: String,
    /// The markdown emphasis delimiter to apply to the header, e.g. "**" for bold.
    emph: String,
}

/// A preprocessor for automatically numbering theorems, lemmas, etc.
pub struct NumThmPreprocessor {
    /// The list of environments handled by the preprocessor.
    envs: Vec<Env>,
    /// Whether theorem numbers must be prefixed by the section number.
    with_prefix: bool,
}

/// The `LabelInfo` structure contains information for formatting the hyperlink to a specific theorem, lemma, etc.
#[derive(Debug, PartialEq)]
struct LabelInfo {
    /// The "numbered name" associated with the label, e.g. "Theorem 1.2.1".
    num_name: String,
    /// The path to the file containing the environment with the label.
    path: PathBuf,
    /// An optional title.
    title: Option<String>,
}

impl NumThmPreprocessor {
    pub fn new(ctx: &PreprocessorContext) -> Self {
        let mut pre = Self::default();

        if let Some(toml::Value::Boolean(b)) = ctx.config.get("preprocessor.numthm.prefix") {
            pre.with_prefix = *b;
        }

        if let Some(toml::Value::Array(array)) = ctx.config.get("preprocessor.numthm.custom_environments") {
            for array_entry in array {
                if let toml::Value::Array(env_params) = array_entry {
                    if let [toml::Value::String(key), toml::Value::String(name), toml::Value::String(emph)] = &env_params[0..3] {
                        pre.envs.push(Env {
                            key: key.to_string(),
                            name: name.to_string(),
                            emph: emph.to_string(),
                        })
                    }
                }
            }
        }

        pre
    }
}

impl Default for NumThmPreprocessor {
    fn default() -> Self {
        let thm: Env = Env {
            key: "thm".to_string(),
            name: "Theorem".to_string(),
            emph: "**".to_string(),
        };

        let lem: Env = Env {
            key: "lem".to_string(),
            name: "Lemma".to_string(),
            emph: "**".to_string(),
        };

        let prop: Env = Env {
            key: "prop".to_string(),
            name: "Proposition".to_string(),
            emph: "**".to_string(),
        };

        let def: Env = Env {
            key: "def".to_string(),
            name: "Definition".to_string(),
            emph: "**".to_string(),
        };

        let rem: Env = Env {
            key: "rem".to_string(),
            name: "Remark".to_string(),
            emph: "*".to_string(),
        };

        Self {
            envs: vec![thm, lem, prop, def, rem],
            with_prefix: false,
        }
    }
}

impl Preprocessor for NumThmPreprocessor {
    fn name(&self) -> &str {
        NAME
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        // a hashmap mapping labels to `LabelInfo` structs
        let mut refs: HashMap<String, LabelInfo> = HashMap::new();

        book.for_each_mut(|item: &mut BookItem| {
            if let BookItem::Chapter(chapter) = item {
                if !chapter.is_draft_chapter() {
                    // one can safely unwrap chapter.path which must be Some(...)
                    let prefix = if self.with_prefix {
                        match &chapter.number {
                            Some(sn) => sn.to_string(),
                            None => String::new(),
                        }
                    } else {
                        String::new()
                    };
                    let path = chapter.path.as_ref().unwrap();
                    for env in &self.envs {
                        chapter.content =
                            find_and_replace_envs(&chapter.content, &prefix, path, env, &mut refs);
                    }
                }
            }
        });

        book.for_each_mut(|item: &mut BookItem| {
            if let BookItem::Chapter(chapter) = item {
                if !chapter.is_draft_chapter() {
                    // one can safely unwrap chapter.path which must be Some(...)
                    let path = chapter.path.as_ref().unwrap();
                    chapter.content = find_and_replace_refs(&chapter.content, path, &refs);
                }
            }
        });

        Ok(book)
    }
}

/// Finds all patterns `{{key}}{mylabel}[mytitle]` where `key` is the key field of `env` (e.g. `thm`)
/// and replaces them with a header (including the title if a title `mytitle` is provided)
/// and potentially an anchor if a label `mylabel` is provided;
/// if a label is provided, it updates the hashmap `refs` with an entry (label, LabelInfo)
/// allowing to format links to the theorem.
fn find_and_replace_envs(
    s: &str,
    prefix: &str,
    path: &Path,
    env: &Env,
    refs: &mut HashMap<String, LabelInfo>,
) -> String {
    let mut ctr = 0;

    let key = &env.key;
    let name = &env.name;
    let emph = &env.emph;

    let mut pattern = r"\{\{".to_string();
    pattern.push_str(key);
    pattern.push_str(r"\}\}(\{(?P<label>.*?)\})?(\[(?P<title>.*?)\])?");
    // see https://regex101.com/ for an explanation of the regex "\{\{key\}\}\{(?P<label>.*?)\}(\[(?P<title>.*?)\])?"
    // matches {{key}}{label}[title] where {label} and [title] are optional
    let re: Regex = Regex::new(pattern.as_str()).unwrap();

    re.replace_all(s, |caps: &regex::Captures| {
        ctr += 1;
        let anchor = match caps.name("label") {
            Some(match_label) => {
                // if a label is given, we must update the hashmap
                let label = match_label.as_str().to_string();
                if refs.contains_key(&label) {
                    // if the same label has already been used we emit a warning and don't update the hashmap
                    warn!("{name} {prefix}{ctr}: Label `{label}' already used");
                } else {
                    refs.insert(
                        label.clone(),
                        LabelInfo {
                            num_name: format!("{name} {prefix}{ctr}"),
                            path: path.to_path_buf(),
                            title: caps.name("title").map(|t| t.as_str().to_string()),
                        },
                    );
                }
                format!("<a name=\"{label}\"></a>\n")
            }
            None => String::new(),
        };
        let header = match caps.name("title") {
            Some(match_title) => {
                let title = match_title.as_str().to_string();
                format!("{emph}{name} {prefix}{ctr} ({title}).{emph}")
            }
            None => {
                format!("{emph}{name} {prefix}{ctr}.{emph}")
            }
        };
        format!("{anchor}{header}")
    })
    .to_string()
}

/// Finds and replaces all patterns {{ref: label}} where label is an existing key in hashmap `refs`
/// with a link towards the relevant theorem.
fn find_and_replace_refs(
    s: &str,
    chap_path: &PathBuf,
    refs: &HashMap<String, LabelInfo>,
) -> String {
    // see https://regex101.com/ for an explanation of the regex
    let re: Regex = Regex::new(r"\{\{(?P<reftype>ref:|tref:)\s*(?P<label>.*?)\}\}").unwrap();

    re.replace_all(s, |caps: &regex::Captures| {
        let label = caps.name("label").unwrap().as_str().to_string();
        if refs.contains_key(&label) {
            let text = match caps.name("reftype").unwrap().as_str() {
                "ref:" => &refs.get(&label).unwrap().num_name,
                _ => {
                    // this must be tref if there is a match
                    match &refs.get(&label).unwrap().title {
                        Some(t) => t,
                        // fallback to the numbered name in case the label does not have an associated title
                        None => &refs.get(&label).unwrap().num_name,
                    }
                }
            };
            let path_to_ref = &refs.get(&label).unwrap().path;
            let rel_path = compute_rel_path(chap_path, path_to_ref);
            format!("[{text}]({rel_path}#{label})")
        } else {
            warn!("Unknown reference: {}", label);
            "**[??]**".to_string()
        }
    })
    .to_string()
}

/// Computes the relative path from the folder containing `chap_path` to the file `path_to_ref`.
fn compute_rel_path(chap_path: &PathBuf, path_to_ref: &PathBuf) -> String {
    if chap_path == path_to_ref {
        return "".to_string();
    }
    let mut local_chap_path = chap_path.clone();
    local_chap_path.pop();
    format!(
        "{}",
        diff_paths(path_to_ref, &local_chap_path).unwrap().display()
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use lazy_static::lazy_static;

    const SECNUM: &str = "1.2.";

    lazy_static! {
        static ref THM: Env = Env {
            key: "thm".to_string(),
            name: "Theorem".to_string(),
            emph: "**".to_string(),
        };
        static ref PROP: Env = Env {
            key: "prop".to_string(),
            name: "Proposition".to_string(),
            emph: "**".to_string(),
        };
        static ref PATH: PathBuf = "crypto/groups.md".into();
    }

    #[test]
    fn wo_label_wo_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let expected = String::from("**Proposition 1.2.1.**");
        assert_eq!(output, expected);
        assert!(refs.is_empty());
    }

    #[test]
    fn with_label_wo_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}{prop:lagrange}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let expected = String::from(
            "<a name=\"prop:lagrange\"></a>\n\
            **Proposition 1.2.1.**",
        );
        assert_eq!(output, expected);
        assert_eq!(refs.len(), 1);
        assert_eq!(
            *refs.get("prop:lagrange").unwrap(),
            LabelInfo {
                num_name: "Proposition 1.2.1".to_string(),
                path: "crypto/groups.md".into(),
                title: None,
            }
        )
    }

    #[test]
    fn wo_label_with_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}[Lagrange Theorem]");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let expected = String::from("**Proposition 1.2.1 (Lagrange Theorem).**");
        assert_eq!(output, expected);
        assert!(refs.is_empty());
    }

    #[test]
    fn with_label_with_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem]");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let expected = String::from(
            "<a name=\"prop:lagrange\"></a>\n\
            **Proposition 1.2.1 (Lagrange Theorem).**",
        );
        assert_eq!(output, expected);
    }

    #[test]
    fn double_label() {
        let mut refs = HashMap::new();
        let input = String::from(
            r"{{prop}}{prop:lagrange}[Lagrange Theorem] {{thm}}{prop:lagrange}[Another Lagrange Theorem]",
        );
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let output = find_and_replace_envs(&output, SECNUM, &PATH, &THM, &mut refs);
        let expected = String::from(
            "<a name=\"prop:lagrange\"></a>\n\
            **Proposition 1.2.1 (Lagrange Theorem).** \
            <a name=\"prop:lagrange\"></a>\n\
            **Theorem 1.2.1 (Another Lagrange Theorem).**",
        );
        assert_eq!(output, expected);
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn label_and_ref_in_same_file() {
        let mut refs = HashMap::new();
        let input =
            String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem] {{ref: prop:lagrange}}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &PROP, &mut refs);
        let output = find_and_replace_refs(&output, &PATH, &refs);
        let expected = String::from(
            "<a name=\"prop:lagrange\"></a>\n\
            **Proposition 1.2.1 (Lagrange Theorem).** \
            [Proposition 1.2.1](#prop:lagrange)",
        );
        assert_eq!(output, expected);
    }

    #[test]
    fn label_and_ref_in_different_files() {
        let mut refs = HashMap::new();
        let label_file: PathBuf = "math/groups.md".into();
        let ref_file: PathBuf = "crypto/bls_signatures.md".into();
        let label_input = String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem]");
        let ref_input = String::from(r"{{ref: prop:lagrange}}");
        let _label_output =
            find_and_replace_envs(&label_input, SECNUM, &label_file, &PROP, &mut refs);
        let ref_output = find_and_replace_refs(&ref_input, &ref_file, &refs);
        let expected = String::from("[Proposition 1.2.1](../math/groups.md#prop:lagrange)");
        assert_eq!(ref_output, expected);
    }

    #[test]
    fn label_and_ref_in_different_files_2() {
        let mut refs = HashMap::new();
        let label_file: PathBuf = "math/algebra/groups.md".into();
        let ref_file: PathBuf = "math/crypto//signatures/bls_signatures.md".into();
        let label_input = String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem]");
        let ref_input = String::from(r"{{ref: prop:lagrange}}");
        let _label_output =
            find_and_replace_envs(&label_input, SECNUM, &label_file, &PROP, &mut refs);
        let ref_output = find_and_replace_refs(&ref_input, &ref_file, &refs);
        let expected = String::from("[Proposition 1.2.1](../../algebra/groups.md#prop:lagrange)");
        assert_eq!(ref_output, expected);
    }

    #[test]
    fn title_ref() {
        let mut refs = HashMap::new();
        let label_file: PathBuf = "math/algebra/groups.md".into();
        let ref_file: PathBuf = "math/crypto//signatures/bls_signatures.md".into();
        let label_input = String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem]");
        let ref_input = String::from(r"{{tref: prop:lagrange}}");
        let _label_output =
            find_and_replace_envs(&label_input, SECNUM, &label_file, &PROP, &mut refs);
        let ref_output = find_and_replace_refs(&ref_input, &ref_file, &refs);
        let expected = String::from("[Lagrange Theorem](../../algebra/groups.md#prop:lagrange)");
        assert_eq!(ref_output, expected);
    }

    #[test]
    fn title_ref_without_title() {
        let mut refs = HashMap::new();
        let label_file: PathBuf = "math/algebra/groups.md".into();
        let ref_file: PathBuf = "math/crypto//signatures/bls_signatures.md".into();
        let label_input = String::from(r"{{prop}}{prop:lagrange}");
        let ref_input = String::from(r"{{tref: prop:lagrange}}");
        let _label_output =
            find_and_replace_envs(&label_input, SECNUM, &label_file, &PROP, &mut refs);
        let ref_output = find_and_replace_refs(&ref_input, &ref_file, &refs);
        let expected = String::from("[Proposition 1.2.1](../../algebra/groups.md#prop:lagrange)");
        assert_eq!(ref_output, expected);
    }
}
