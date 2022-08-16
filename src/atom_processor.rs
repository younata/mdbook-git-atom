extern crate html_escape;

use git2::Time;
use std::path::PathBuf;
use url::Url;
use atom_syndication::Link;
use chrono::FixedOffset;
use mdbook::book::Book;
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use std::fs;
use crate::post_finder::{Author, Post, PostFinder};

pub struct AtomProcessor;
struct AtomGenerator;

struct AtomConfig {
    title: String,
    base_url: Url,
    content_path: PathBuf,
    root_path: PathBuf,
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

        let post_finder = PostFinder::new(config.root_path.to_str().expect("Create PostFinder"));
        let posts = post_finder.search(&book, &config.content_path, Some(config.maximum_number_of_lines), config.target_number_of_entries);

        let generator = AtomGenerator {};
        let feed = generator.generate(posts, config.title, config.base_url);

        let feed_path: PathBuf = config.content_path.join("atom.xml");
        fs::write(feed_path, feed.to_string()).expect("Write atom.xml");

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl AtomGenerator {
    fn generate(&self, posts: Vec<Post>, title: String, base_url: Url) -> atom_syndication::Feed {
        let entries: Vec<atom_syndication::Entry> = posts
            .iter()
            .filter_map(|p| p.to_atom_entry(&base_url))
            .collect();

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

    fn link(&self, base_url: &Url) -> Option<atom_syndication::Link> {
        if let Some(url_string) = self.source_url(Some(base_url)) {
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
                value: Some(html_escape::encode_text(&self.content.as_ref().unwrap_or(&"".to_string())).to_string()),
                src: None,
                content_type: Some("html".to_string())
            }),
            extensions: Default::default()
        })
    }
}