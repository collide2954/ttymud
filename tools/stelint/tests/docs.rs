use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use stelint::{Rules, check, check_html};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn markdown_files(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if name != ".git" && name != "target" {
                markdown_files(&path, found)?;
            }
        } else if path.extension().is_some_and(|ext| ext == "md") {
            found.push(path);
        }
    }
    Ok(())
}

#[test]
fn all_documents_follow_the_style_guide() -> io::Result<()> {
    let root = repository_root();
    let rules = Rules::from_style_guide(&fs::read_to_string(root.join("docs/style.md"))?);
    assert!(!rules.is_empty(), "docs/style.md has no word table");
    let mut files = Vec::new();
    markdown_files(&root, &mut files)?;
    files.sort();
    let mut problems = Vec::new();
    for path in &files {
        let name = path.strip_prefix(&root).unwrap_or(path).display();
        for issue in check(&fs::read_to_string(path)?, &rules) {
            problems.push(format!("{name}:{}: {}", issue.line, issue.message));
        }
    }
    assert_eq!(problems, Vec::<String>::new());
    Ok(())
}

#[test]
fn all_site_pages_follow_the_style_guide() -> io::Result<()> {
    let root = repository_root();
    let rules = Rules::from_style_guide(&fs::read_to_string(root.join("docs/style.md"))?);
    let mut problems = Vec::new();
    let mut pages = 0;
    for entry in fs::read_dir(root.join("site"))? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "html") {
            continue;
        }
        pages += 1;
        let name = path.strip_prefix(&root).unwrap_or(&path).display();
        for issue in check_html(&fs::read_to_string(&path)?, &rules) {
            problems.push(format!("{name}:{}: {}", issue.line, issue.message));
        }
    }
    assert!(pages > 0, "site/ has no pages");
    assert_eq!(problems, Vec::<String>::new());
    Ok(())
}
