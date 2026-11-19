use std::fs;
use std::io;
use std::path::Path;

use ttymud_game::{Content, World};

type Mismatch = (String, Vec<String>, Vec<String>);

fn replay(content: &Content, transcript: &str) -> Vec<String> {
    let mut world = World::new(content.clone());
    let (player, joined) = world.join("ghost");
    let mut lines: Vec<String> = joined.iter().map(|out| out.message.plain()).collect();
    for command in transcript
        .lines()
        .filter_map(|line| line.strip_prefix("> "))
    {
        lines.push(format!("> {command}"));
        let outputs = world.command(player, command);
        lines.extend(outputs.iter().map(|out| out.message.plain()));
    }
    lines
}

fn expected(transcript: &str) -> Vec<String> {
    transcript
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn scenarios_match_their_transcripts() -> io::Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let city = fs::read_to_string(dir.join("city.ini"))?;
    let content =
        Content::parse(&city).map_err(|errors| io::Error::other(format!("{errors:?}")))?;
    let mut paths = fs::read_dir(&dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "txt"));
    paths.sort();
    assert!(!paths.is_empty(), "no scenarios in {}", dir.display());
    let mut mismatches: Vec<Mismatch> = Vec::new();
    for path in &paths {
        let transcript = fs::read_to_string(path)?;
        let (want, got) = (expected(&transcript), replay(&content, &transcript));
        if want != got {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            mismatches.push((name.into_owned(), want, got));
        }
    }
    assert_eq!(mismatches, Vec::<Mismatch>::new());
    Ok(())
}
