use clap::{App, Arg, SubCommand};
use mdbook_git_atom::library_helpers;
use mdbook_git_atom::atom_processor::AtomProcessor;

pub fn make_app() -> App<'static, 'static> {
    App::new("mdbook-git-atom")
        .about("A preprocessor that generates an atom feed for the html renderer")
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() {
    let matches = make_app().get_matches();
    let preprocessor = AtomProcessor;
    if let Some(sub_args) = matches.subcommand_matches("supports") {
        library_helpers::handle_supports(&preprocessor, sub_args);
    }
    if let Err(e) = library_helpers::handle_preprocessing(&preprocessor) {
        eprintln!("{}", e);
    }
}