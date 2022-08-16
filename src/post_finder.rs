use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use git2::{Blame, BlameOptions, Repository, Time};
use mdbook::book::Book;
use mdbook::BookItem;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;
use url::Url;

#[derive(PartialEq, Eq, Hash)]
pub struct Author {
    pub(crate) name: String,
    pub(crate) email: Option<String>
}

pub struct Post {
    pub(crate) path: PathBuf,
    pub(crate) last_modified_date: Time,
    pub(crate) created_date: Time,
    pub(crate) authors: HashSet<Author>,
    pub(crate) title: String,
    pub(crate) id: String,
    pub(crate) content: Option<String>,
}

pub struct PostFinder {
    repo: Repository
}

impl PostFinder {
    pub fn new(repository_path: &str) -> PostFinder {
        let repo = match Repository::open(repository_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };

        PostFinder { repo }
    }

    pub fn search(&self, book: &Book, content_path: &PathBuf, max_number_of_lines: Option<i64>, target_entries: i64) -> Vec<Post> {
        let mut posts: Vec<Post> = book
            .iter()
            .filter_map({ |item|
                if let BookItem::Chapter(chapter) = item {
                    let path = content_path.join(chapter.source_path.as_ref()?.as_path());
                    self.post(path, chapter.name.to_string(), chapter.path.as_ref()?.to_path_buf(), max_number_of_lines)
                } else {
                    None
                }
            })
            .collect();
        posts.sort_by( |a, b| a.last_modified_date.cmp(&b.last_modified_date).reverse());
        self.most_recent(posts, target_entries)
    }

    fn most_recent(&self, posts: Vec<Post>, target_entries: i64) -> Vec<Post> {
        // get min_commits newest commit.
        let mut revwalk = self.repo.revwalk().expect("Unable to create revwalk");
        revwalk.set_sorting(git2::Sort::TIME).expect("Unable to sort the revwalk");
        revwalk.push_head().expect("Unable to push head to the revwalk");
        let walk = revwalk
            .filter_map(|id| {
                let id = id.ok()?;
                let commit = self.repo.find_commit(id).ok()?;
                Some(commit)
            }).into_iter();
        let commit = walk
            .last()
            .expect("No commits to take from");

        let oldest_date = commit.time();

        let entries = posts
            .into_iter()
            .filter(|post| post.last_modified_date >= oldest_date);

        return if target_entries > 0 {
            entries.take(target_entries as usize).collect()
        } else {
            entries.collect()
        }

    }

    fn post(&self, path: PathBuf, title: String, content_path: PathBuf, number_of_lines: Option<i64>) -> Option<Post> {
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

        let content: Option<String>;
        if let Some(number_of_lines) = number_of_lines {
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

            let mut content_string = String::new();
            html::push_html(&mut content_string, parser);
            content = Some(content_string);
        } else {
            content = None;
        }

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

impl Post {
    pub fn source_url(&self, base_url: Option<&Url>) -> Option<String> {
        let url_string: String;
        if let Some(base_url) = base_url {
            url_string = base_url.join(self.path.to_str()?)
                .ok()?
                .to_string();
        } else {
            url_string = self.path.to_str().unwrap().to_string()
        }

        Some(url_by_replacing_md_suffix(url_by_replacing_readme_md(url_string)))
    }
}

fn url_by_replacing_md_suffix(url_string: String) -> String {
    let re = Regex::new(r"md$").unwrap();
    re.replace_all(url_string.as_str(), "html").to_string()
}

fn url_by_replacing_readme_md(url_string: String) -> String {
    let re = Regex::new(r"README.md$").unwrap();
    re.replace_all(url_string.as_str(), "index.html").to_string()
}