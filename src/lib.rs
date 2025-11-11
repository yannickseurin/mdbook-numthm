//! An [mdBook](https://github.com/rust-lang/mdBook) preprocessor for automatically numbering theorems, lemmas, etc.

use log::warn;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use pathdiff::diff_paths;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

/// The preprocessor name.
const NAME: &str = "numthm";

/// An environment handled by the preprocessor.
#[derive(Debug, Clone, Deserialize)]
struct Env {
    /// The name to display in the header, e.g. "Theorem".
    #[serde(default = "Env::name_default")]
    name: String,
    /// The markdown emphasis delimiter to apply to the header, e.g. "**" for bold.
    #[serde(default = "Env::emph_default")]
    emph: String,
}

impl Env {
    fn create(name: &str, emph: &str) -> Self {
        Env {
            name: name.to_string(),
            emph: emph.to_string(),
        }
    }
    fn name_default() -> String {
        String::from("Environment")
    }
    fn emph_default() -> String {
        String::from("**")
    }
}

/// Environment collection
#[derive(Debug, Clone, Deserialize)]
struct EnvMap(HashMap<String, Env>);

impl Default for EnvMap {
    fn default() -> Self {
        let mut envs: HashMap<String, Env> = HashMap::new();
        envs.insert("thm".to_string(), Env::create("Theorem", "**"));
        envs.insert("lem".to_string(), Env::create("Lemma", "**"));
        envs.insert("prop".to_string(), Env::create("Proposition", "**"));
        envs.insert("def".to_string(), Env::create("Definition", "**"));
        envs.insert("rem".to_string(), Env::create("Remark", "*"));
        EnvMap(envs)
    }
}

impl Deref for EnvMap {
    type Target = HashMap<String, Env>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for EnvMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
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

/// A preprocessor for automatically numbering theorems, lemmas, etc.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NumThmPreprocessor {
    /// The list of environments handled by the preprocessor.
    environments: EnvMap,
    /// Whether theorem numbers must be prefixed by the section number.
    with_prefix: bool,
}

impl NumThmPreprocessor {
    pub fn new(ctx: &PreprocessorContext) -> Self {
        let mut config = Self::default();

        let toml_config: &toml::value::Table = ctx.config.get_preprocessor("numthm").unwrap();

        // Set use of prefix conf.
        if let Some(b) = toml_config.get("prefix").and_then(toml::Value::as_bool) {
            config.with_prefix = b;
        }

        // Get environments table
        if let Some(envs) = toml_config
            .get("environments")
            .and_then(toml::Value::as_table)
        {
            for (key, value) in envs.iter() {
                // Update from entries, but only if data is available
                if let Some(entry) = toml::Value::as_table(value) {
                    // Allow removal of entry
                    if let Some(ignore) = entry.get("ignore").and_then(toml::Value::as_bool) {
                        if ignore {
                            config.environments.remove(key);
                            continue;
                        }
                    }

                    let name = entry.get("name").and_then(toml::Value::as_str);
                    let emph = entry.get("emph").and_then(toml::Value::as_str);

                    if let Some(env) = config.environments.get_mut(key) {
                        if let Some(v) = name {
                            env.name = v.to_string();
                        }

                        if let Some(v) = emph {
                            env.emph = v.to_string();
                        }
                    } else {
                        config.environments.insert(
                            String::from(key),
                            Env::create(name.unwrap_or("Environment"), emph.unwrap_or("**")),
                        );
                    }
                }
            }
        }

        config
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
                    chapter.content = find_and_replace_envs(
                        &chapter.content,
                        &prefix,
                        path,
                        &self.environments,
                        &mut refs,
                    );
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
    envs: &EnvMap,
    refs: &mut HashMap<String, LabelInfo>,
) -> String {
    let mut counter: HashMap<String, u32> = envs.iter().map(|(k, _)| (k.clone(), 0)).collect();

    let keys = envs
        .keys()
        .map(String::as_str)
        .collect::<Vec<&str>>()
        .join("|");
    let pattern = format!(
        r"\{{\{{(?P<key>{})\}}\}}(\{{(?P<label>.*?)\}})?(\[(?P<title>.*?)\])?",
        keys
    );
    // see https://regex101.com/ for an explanation of the regex "\{\{(?P<key>key1|key2)\}\}(\{(?P<label>.*?)\})?(\[(?P<title>.*?)\])?"
    // matches {{key}}{label}[title] where {label} and [title] are optional
    let re: Regex = Regex::new(pattern.as_str()).unwrap();

    re.replace_all(s, |caps: &regex::Captures| {
        // key must have been matched
        let key = caps.name("key").unwrap().as_str();

        // key is absolutely part of env, so unwrap should be ok
        let env = envs.get(key).unwrap();
        let name = &env.name;
        let emph = &env.emph;
        let ctr = counter.get_mut(key).unwrap();
        *ctr += 1;

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
        static ref ENVMAP: EnvMap = EnvMap::default();
        static ref PATH: PathBuf = "crypto/groups.md".into();
    }

    #[test]
    fn wo_label_wo_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
        let expected = String::from("**Proposition 1.2.1.**");
        assert_eq!(output, expected);
        assert!(refs.is_empty());
    }

    #[test]
    fn wo_label_wo_title_replace_default() {
        let mut env_map = EnvMap::default();
        env_map.insert(String::from("prop"), Env::create("Proposal", "*"));
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &env_map, &mut refs);
        let expected = String::from("*Proposal 1.2.1.*");
        assert_eq!(output, expected);
        assert!(refs.is_empty());
    }

    #[test]
    fn with_label_wo_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}{prop:lagrange}");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
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
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
        let expected = String::from("**Proposition 1.2.1 (Lagrange Theorem).**");
        assert_eq!(output, expected);
        assert!(refs.is_empty());
    }

    #[test]
    fn with_label_with_title() {
        let mut refs = HashMap::new();
        let input = String::from(r"{{prop}}{prop:lagrange}[Lagrange Theorem]");
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
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
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
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
        let output = find_and_replace_envs(&input, SECNUM, &PATH, &ENVMAP, &mut refs);
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
            find_and_replace_envs(&label_input, SECNUM, &label_file, &ENVMAP, &mut refs);
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
            find_and_replace_envs(&label_input, SECNUM, &label_file, &ENVMAP, &mut refs);
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
            find_and_replace_envs(&label_input, SECNUM, &label_file, &ENVMAP, &mut refs);
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
            find_and_replace_envs(&label_input, SECNUM, &label_file, &ENVMAP, &mut refs);
        let ref_output = find_and_replace_refs(&ref_input, &ref_file, &refs);
        let expected = String::from("[Proposition 1.2.1](../../algebra/groups.md#prop:lagrange)");
        assert_eq!(ref_output, expected);
    }
}
