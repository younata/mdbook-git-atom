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
}

struct AtomConfig {
    title: String,
    base_url: Url,
    content_path: PathBuf,
    root_path: PathBuf,
    // Create enough posts to cover the recent number of commits. Defaults to 10.
    minimum_number_of_commits: i64,
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
        let mut minimum_number_of_commits: &i64 = &10;
        if let Some(toml::Value::Integer(min_commits)) = section_config.get("minimum_number_of_commits") {
            minimum_number_of_commits = min_commits;
        }

        Some(AtomConfig {
            title: ctx.config.book.title.as_ref()?.to_string(),
            base_url: Url::parse(base_url_str).ok()?,
            content_path: ctx.config.book.src.to_path_buf(),
            root_path: ctx.root.to_path_buf(),
            minimum_number_of_commits: *minimum_number_of_commits
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
                    generator.post(path, chapter.name.to_string(), chapter.path.as_ref()?.to_path_buf())
                } else {
                    None
                }
            })
            .collect();

        let feed = generator.generate(posts, config.title, config.base_url, config.minimum_number_of_commits);

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

    fn generate(&self, mut posts: Vec<Post>, title: String, base_url: Url, min_commits: i64) -> atom_syndication::Feed {
        // self.repo.log
        posts.sort_by( |a, b| a.last_modified_date.cmp(&b.last_modified_date).reverse());

        // get min_commits newest commit.
        let mut revwalk = self.repo.revwalk().expect("Unable to create revwalk");
        revwalk.set_sorting(git2::Sort::TIME).expect("Unable to sort the revwalk");
        revwalk.push_head().expect("Unable to push head to the revwalk");
        let commit: Commit = revwalk
            .filter_map(|id| {
                let id = id.ok()?;
                let commit = self.repo.find_commit(id).ok()?;
                Some(commit)
            })
            .take(min_commits as usize)
            .last().expect("No commits to take from");
        let oldest_date = commit.time();

        let entries: Vec<atom_syndication::Entry> = posts
            .iter()
            .filter(|post| post.last_modified_date <= oldest_date)
            .filter_map(|p| p.to_atom_entry(&base_url))
            .collect();

        atom_syndication::Feed {
            title: atom_syndication::Text {
                value: title,
                base: None,
                lang: None,
                r#type: Default::default()
            },
            id: "".to_string(),
            updated: fixed_date_time_from_timestamp(&posts.get(0).expect("No posts to get a last updated at from").last_modified_date),
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

    fn post(&self, path: PathBuf, title: String, content_path: PathBuf) -> Option<Post> {
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

        Some(Post {
            path: content_path,
            last_modified_date: last_modified,
            created_date: created_at,
            authors,
            title,
            id: id.to_string()
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
                value: None,
                src: self.source_url(base_url),
                content_type: Some("html".to_string())
            }),
            extensions: Default::default()
        })
    }
}