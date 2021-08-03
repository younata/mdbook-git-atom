# mdbook-git-atom

[![CI status](https://ci.younata.com/api/v1/pipelines/knowledge/jobs/mdbook-git-atom/badge)](https://ci.younata.com/teams/main/pipelines/knowledge/jobs/mdbook-git-atom/)
[![Latest Version](https://img.shields.io/crates/v/mdbook-git-atom.svg)](https://crates.io/crates/mdbook-git-atom)

Generate an Atom feed from the git log of your [mdBook](https://github.com/rust-lang/mdbook)

## Configuration

The only configuration `mdbook-git-atom` requires is a base url for your book.

Additionally, you can optionally specify how many lines to include in the article preview with the `article_preview_lines`.
-1 means the full text of the article will be included. 0 will mean that no article preview will be included (same as if it hadn't been specified).
Other positive values will use the first number of lines as the article preview (or the entire article, whichever is lower).
Obviously, this can massively increase the size of the generated article.

```toml
[preprocessor.git-atom]
base_url = "https://example.com"
article_preview_lines = 0
```

The rest is figured out from the content's markdown.

The atom feed is placed in the content path (same place as `SUMMARY.md`) as `atom.xml`. I advise modifying your templates to insert `<link rel="alternate" type="application/atom+xml" title="Atom" href="/atom.xml">` in the `<head>` section of your html, but this isn't necessary.

It might be best to place this at the end of your list of preprocessors, as mdbook invokes preprocessors in the order they are specified in.
