use clap::{crate_version, Arg, ArgMatches, Command};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook::errors::{Error, Result};
use semver::{Version, VersionReq};
use mdbook_numthm::NumThmPreprocessor;
use std::io;

/// Parse CLI options.
pub fn make_app() -> Command {
    Command::new("mdbook-numthm")
        .version(crate_version!())
        .about("An mdbook preprocessor that provides numbered theorems, lemmas, etc..")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn handle_preprocessing() -> Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let pre = NumThmPreprocessor::new(&ctx);

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(sub_args: &ArgMatches) -> Result<()> {
    let renderer = sub_args
        .get_one::<String>("renderer")
        .expect("Required argument");

    let pre = NumThmPreprocessor::default();

    let supported = pre.supports_renderer(renderer);
    
    if supported {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "The {} preprocessor does not support the '{}' renderer",
            pre.name(),
            renderer,
        )))
    }
}

fn main() -> Result<()> {
    ::std::env::set_var("RUST_LOG", "warn");
    env_logger::init();
    let matches = make_app().get_matches();

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        // handle cmdline supports
        handle_supports(sub_args)
    } else {
        // handle preprocessing
        handle_preprocessing()
    }
}
