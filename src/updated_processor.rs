use std::path::PathBuf;
use chrono::FixedOffset;
use mdbook::book::Book;
use mdbook::BookItem;
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use regex::{Captures, Regex};
use crate::post_finder::{Post, PostFinder};

pub struct UpdatedProcessor;

struct UpdatedConfig {
    content_path: PathBuf,
    root_path: PathBuf,
    // Target number of entries in the atom feed to create. Defaults to 10.
    // Set this to 0 to get the old behavior where minimum_number_of_commits is paid attention to.
    // This basically overrides minimum_number_of_commits when it's a positive number.
    // We'll search as far back as necessary to create the target amount of entries.
    target_number_of_entries: i64,
}

impl UpdatedConfig {
    fn from_book_config(ctx: &PreprocessorContext, name: &str) -> Option<UpdatedConfig> {
        let section_config = ctx.config.get_preprocessor(name)?;

        let mut target_number_of_entries: &i64 = &10;
        if let Some(toml::Value::Integer(target_entries)) = section_config.get("target_number_of_entries") {
            if (*target_entries) < -1 {
                panic!("Invalid target number of entries provided: {}. Expected 0 or a positive number.", target_entries);
            }
            target_number_of_entries = target_entries;
        }

        Some(UpdatedConfig {
            content_path: ctx.config.book.src.to_path_buf(),
            root_path: ctx.root.to_path_buf(),
            target_number_of_entries: *target_number_of_entries,
        })
    }
}

impl Preprocessor for UpdatedProcessor {
    fn name(&self) -> &str {
        "git-updated"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let config = UpdatedConfig::from_book_config(&ctx, self.name()).expect("Create recently updated configuration");

        let post_finder = PostFinder::new(config.root_path.to_str().expect("Create PostFinder"));
        let posts = post_finder.search(&book, &config.content_path, None, config.target_number_of_entries);

        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                chapter.content = self.process_chapter(&chapter.content, &posts);
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

impl UpdatedProcessor {
    fn process_chapter(&self, content: &str, posts: &Vec<Post>) -> String {
        // let regex = Regex::new(r"^(?P<indent>.*)\{\{#recently_updated}}").unwrap();
        let regex = Regex::new(r"\{\{#recently_updated}}").unwrap();

        let captures: Vec<Captures> = regex.captures_iter(&content).collect();

        let mut processed_content = String::new();

        let mut last_endpoint: usize = 0;

        for capture in captures {
            let full_match = capture.get(0).unwrap();

            // if let Some(indentation) = capture.name("indent") {
                processed_content.push_str(&content[last_endpoint..full_match.start()]);

                last_endpoint = full_match.end();
                processed_content.push_str(self.generate_markdown(posts, "").as_str());

            // processed_content.push_str(self.generate_markdown(posts, indentation.as_str()).as_str());
            // }
        }

        if content.len() > last_endpoint {
            processed_content.push_str(&content[last_endpoint..content.len()]);
        }

        processed_content
    }

    fn generate_markdown(&self, posts: &Vec<Post>, indentation_prefix: &str) -> String {
        posts.iter()
            .map({ |post|
                format!("{}{}", indentation_prefix, post.list_link())
            })
            .fold(String::new(), |a, b| a + &b + "\n")
    }
}

impl Post {
    fn list_link(&self) -> String {
        let last_modified_naivedatetime = chrono::NaiveDateTime::from_timestamp(self.last_modified_date.seconds(), 0);

        let last_modified_datetime = chrono::DateTime::<FixedOffset>::from_utc(last_modified_naivedatetime, chrono::FixedOffset::east(0));
        format!("- [{}](/{}) ({})", self.title, self.path.to_str().expect("Actual path"), last_modified_datetime.format("%Y-%m-%d"))
    }
}