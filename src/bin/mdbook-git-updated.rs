use clap::{App, Arg, SubCommand};
use mdbook_git_atom::library_helpers;
use mdbook_git_atom::updated_processor::UpdatedProcessor;

pub fn make_app() -> App<'static, 'static> {
    App::new("mdbook-git-updated")
        .about("A preprocessor that replaces {{#recently_updated}} with the paths to the 10 most recently updated pages in the repo.")
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() {
    let matches = make_app().get_matches();
    let preprocessor = UpdatedProcessor;
    if let Some(sub_args) = matches.subcommand_matches("supports") {
        library_helpers::handle_supports(&preprocessor, sub_args);
    }
    if let Err(e) = library_helpers::handle_preprocessing(&preprocessor) {
        eprintln!("{}", e);
    }
}