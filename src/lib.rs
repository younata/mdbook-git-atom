extern crate html_escape;

use git2::{Repository, Time, BlameOptions, Commit, Blame};
use std::path::PathBuf;
use std::collections::HashSet;
use url::Url;
use atom_syndication::Link;
use chrono::FixedOffset;
use mdbook::book::Book;
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use std::fs;
use regex::Regex;
use std::fs::File;
use std::io::{BufReader, Read, BufRead};
use pulldown_cmark::{Parser, Options, html};

pub struct AtomProcessor;

pub struct AtomGenerator {
    repo: Repository
}

#[derive(PartialEq, Eq, Hash)]
pub struct Author {
    name: String,
    email: Option<String>
}

pub struct Post {
    path: PathBuf,
    last_modified_date: Time,
    created_date: Time,
    authors: HashSet<Author>,
    title: String,
    id: String,
    content: String,
}

struct AtomConfig {
    title: String,
    base_url: Url,
    content_path: PathBuf,
    root_path: PathBuf,
    // Create enough posts to cover the recent number of commits. Defaults to -1.
    // Deprecated. Please use target_number_of_entries, as that should be consistent regardless of how often you modify a single article.
    minimum_number_of_commits: i64,
    // Max number of lines in the article to include. 0 means no preview, -1 means whole article. Defaults to 0.
    maximum_number_of_lines: i64,
    // Target number of entries in the atom feed to create. Defaults to 10.
    // Set this to 0 to get the old behavior where minimum_number_of_commits is paid attention to.
    // This basically overrides minimum_number_of_commits when it's a positive number.
    // We'll search as far back as necessary to create the target amount of entries.
    target_number_of_entries: i64,
}

impl AtomConfig {
    fn from_book_config(ctx: &PreprocessorContext, name: &str) -> Option<AtomConfig> {
        let section_config = ctx.config.get_preprocessor(name)?;

        let base_url_str: &str;
        if let Some(toml::value::Value::String(base_url)) = section_config.get("base_url") {
            base_url_str = base_url.as_str();
        } else {
            return None
        }
        let mut minimum_number_of_commits: &i64 = &-1;
        if let Some(toml::Value::Integer(min_commits)) = section_config.get("minimum_number_of_commits") {
            minimum_number_of_commits = min_commits;
        }
        let mut article_lines: &i64 = &0;
        if let Some(toml::Value::Integer(max_lines)) = section_config.get("article_preview_lines") {
            if (*max_lines) < -1 {
                panic!("Invalid number of article preview lines specified: {}. Expected 0 or a positive number.", max_lines);
            }
            article_lines = max_lines;
        }
        let mut target_number_of_entries: &i64 = &10;
        if let Some(toml::Value::Integer(target_entries)) = section_config.get("target_number_of_entries") {
            if (*target_entries) < -1 {
                panic!("Invalid target number of entries provided: {}. Expected 0 or a positive number.", target_entries);
            }
            target_number_of_entries = target_entries;
        }

        Some(AtomConfig {
            title: ctx.config.book.title.as_ref()?.to_string(),
            base_url: Url::parse(base_url_str).ok()?,
            content_path: ctx.config.book.src.to_path_buf(),
            root_path: ctx.root.to_path_buf(),
            minimum_number_of_commits: *minimum_number_of_commits,
            maximum_number_of_lines: *article_lines,
            target_number_of_entries: *target_number_of_entries,
        })
    }
}

impl Preprocessor for AtomProcessor {
    fn name(&self) -> &str {
        "git-atom"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        let config = AtomConfig::from_book_config(&ctx, self.name()).expect("Create atom configuration");

        let generator = AtomGenerator::new(config.root_path.to_str().expect("Create atom generator"));

        let posts: Vec<Post> = book
            .iter()
            .filter_map({ |item|
                if let BookItem::Chapter(chapter) = item {
                    let path = config.content_path.join(chapter.source_path.as_ref()?.as_path());
                    generator.post(path, chapter.name.to_string(), chapter.path.as_ref()?.to_path_buf(), config.maximum_number_of_lines)
                } else {
                    None
                }
            })
            .collect();

        let feed = generator.generate(posts, config.title, config.base_url, config.minimum_number_of_commits, config.target_number_of_entries);

        let feed_path: PathBuf = config.content_path.join("atom.xml");
        fs::write(feed_path, feed.to_string()).expect("Write atom.xml");

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl AtomGenerator {
    fn new(repository_path: &str) -> AtomGenerator {
        let repo = match Repository::open(repository_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        AtomGenerator { repo }
    }

    fn generate(&self, mut posts: Vec<Post>, title: String, base_url: Url, min_commits: i64, target_entries: i64) -> atom_syndication::Feed {
        // self.repo.log
        posts.sort_by( |a, b| a.last_modified_date.cmp(&b.last_modified_date).reverse());

        // get min_commits newest commit.
        let mut revwalk = self.repo.revwalk().expect("Unable to create revwalk");
        revwalk.set_sorting(git2::Sort::TIME).expect("Unable to sort the revwalk");
        revwalk.push_head().expect("Unable to push head to the revwalk");
        let commit: Commit;
        let walk = revwalk
            .filter_map(|id| {
                let id = id.ok()?;
                let commit = self.repo.find_commit(id).ok()?;
                Some(commit)
            }).into_iter();
        if target_entries == 0 {
            commit = walk
                .take(min_commits as usize)
                .last().expect("No commits to take from");
        } else {
            commit = walk
                .last().expect("No commits to take from");
        }
        let oldest_date = commit.time();

        let entries_ish = posts
            .iter()
            .filter(|post| post.last_modified_date >= oldest_date);

        let entries: Vec<atom_syndication::Entry>;

        if target_entries > 0 {
            entries = entries_ish
                .take(target_entries as usize)
                .filter_map(|p| p.to_atom_entry(&base_url))
                .collect();
        } else {
            entries = entries_ish
                .filter_map(|p| p.to_atom_entry(&base_url))
                .collect();
        }

        eprintln!("created {} entries", entries.len());

        if posts.is_empty() {
            panic!("No posts? How?");
        }

        atom_syndication::Feed {
            title: atom_syndication::Text {
                value: title,
                base: None,
                lang: None,
                r#type: Default::default()
            },
            id: "".to_string(),
            updated: fixed_date_time_from_timestamp(
                &posts
                    .get(0)
                    .expect("No posts to get a last updated at from")
                    .last_modified_date
            ),
            authors: vec![],
            categories: vec![],
            contributors: vec![],
            generator: None,
            icon: None,
            links: vec![],
            logo: None,
            rights: None,
            subtitle: None,
            entries,
            extensions: Default::default(),
            namespaces: Default::default()
        }
    }

    fn post(&self, path: PathBuf, title: String, content_path: PathBuf, number_of_lines: i64) -> Option<Post> {
        // Prepare our blame options
        let mut opts = BlameOptions::new();
        opts.track_copies_same_commit_moves(true)
            .track_copies_same_commit_copies(true)
            .first_parent(true);

        let blame_result = self.repo.blame_file(&path.as_path(), Some(&mut opts));

        let blame: Blame;
        match blame_result {
            Ok(bl) => blame = bl,
            Err(_err) => {
                return None
            }
        }

        let mut authors = HashSet::new();
        let last_modified = blame.get_index(0).expect("No blame at index 0").final_signature().when();
        let created_at = blame.get_index(blame.len() - 1).expect("no blame at last index").final_signature().when();

        for hunk in blame.iter() {
            let signature = hunk.final_signature();
            if let Some(name) = signature.name() {
                authors.insert(Author {
                    name: name.to_string(),
                    email: signature.email().map(|email| email.to_string()),
                });
            }
        }

        let id = &content_path.to_str().unwrap_or("").to_string();

        let mut markdown_content: String = String::new();
        let file = File::open(&path).expect("Unable to open file");
        let mut buf_reader = BufReader::new(file);
        if number_of_lines == -1 {
            buf_reader.read_to_string(&mut markdown_content).expect("Wasn't able to read text");
        } else if number_of_lines > 0 {
            markdown_content = buf_reader
                .lines()
                .take(number_of_lines as usize)
                .flat_map(|s| s.ok())
                .collect::<Vec<String>>()
                .join("\n")
                .to_string();
        }

        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(markdown_content.as_str(), options);

        let mut content = String::new();
        html::push_html(&mut content, parser);

        Some(Post {
            path: content_path,
            last_modified_date: last_modified,
            created_date: created_at,
            authors,
            title,
            id: id.to_string(),
            content,
        })
    }
}

fn fixed_date_time_from_timestamp(timestamp: &Time) -> chrono::DateTime<FixedOffset> {
    let naive = chrono::NaiveDateTime::from_timestamp(timestamp.seconds(), 0);

    chrono::DateTime::<FixedOffset>::from_utc(naive, chrono::FixedOffset::east(0))
}

impl Author {
    fn as_person(&self) -> atom_syndication::Person {
        atom_syndication::Person {
            name: self.name.to_string(),
            email: self.email.as_ref().map(|e| e.to_string()),
            uri: None
        }
    }
}

impl Post {
    fn authors_vector(&self) -> Vec<atom_syndication::Person> {
        self.authors.iter()
            .map(|author| author.as_person())
            .collect()
    }

    fn source_url(&self, base_url: &Url) -> Option<String> {
        let url_string = base_url.join(self.path.to_str()?)
            .ok()?
            .to_string();

        let re = Regex::new(r"md$").unwrap();

        Some(re.replace_all(url_string.as_str(), "html").to_string())
    }

    fn link(&self, base_url: &Url) -> Option<atom_syndication::Link> {
        if let Some(url_string) = self.source_url(base_url) {
            Some(Link {
                href: url_string,
                rel: "self".to_string(),
                hreflang: None,
                mime_type: None,
                title: None,
                length: None
            })
        } else {
            None
        }
    }

    fn to_atom_entry(&self, base_url: &Url) -> Option<atom_syndication::Entry> {
        Some(atom_syndication::Entry {
            title: atom_syndication::Text {
                value: self.title.to_string(),
                base: None,
                lang: None,
                r#type: Default::default()
            },
            id: self.id.to_string(),
            updated: fixed_date_time_from_timestamp(&self.last_modified_date),
            authors: self.authors_vector(),
            categories: vec![],
            contributors: vec![],
            links: vec![self.link(base_url)?],
            published: Some(fixed_date_time_from_timestamp(&self.created_date)),
            rights: None,
            source: None,
            summary: None,
            content: Some(atom_syndication::Content {
                base: None,
                lang: None,
                value: Some(html_escape::encode_text(&self.content).to_string()),
                src: None,
                content_type: Some("html".to_string())
            }),
            extensions: Default::default()
        })
    }
}