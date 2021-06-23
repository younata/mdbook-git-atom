# mdbook-git-atom

[![CI status](https://ci.younata.com/api/v1/pipelines/knowledge/jobs/mdbook-git-atom/badge)](https://ci.younata.com/teams/main/pipelines/knowledge/jobs/mdbook-git-atom/)
[![Latest Version](https://img.shields.io/crates/v/mdbook-git-atom.svg)](https://crates.io/crates/mdbook-git-atom)

Generate an Atom feed from the git log of your [mdBook](https://github.com/rust-lang/mdbook)

## Configuration

The only configuration `mdbook-git-atom` requires is a base url for your book.

```toml
[preprocessor.git-atom]
base_url = "https://example.com"
```

The rest is figured out from the content's markdown.

The atom feed is placed in the content path (same place as `SUMMARY.md`) as `atom.xml`. I advise modifying your templates to insert `<link rel="alternate" type="application/atom+xml" title="Atom" href="/atom.xml">` in the `<head>` section of your html, but this isn't necessary.

It might be best to place this at the end of your list of preprocessors.
