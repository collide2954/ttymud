use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

const FILE: &str = "settings.ini";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Line,
    Edit,
    Color,
    Hud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    None,
    Basic,
    Indexed,
    True,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub layer: Layer,
    pub color: Depth,
    pub ascii: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Choices {
    pub layer: Option<Layer>,
    pub color: Option<Depth>,
    pub ascii: Option<bool>,
    pub verbose: Option<bool>,
}

impl FromStr for Layer {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "line" => Ok(Self::Line),
            "edit" => Ok(Self::Edit),
            "color" => Ok(Self::Color),
            "hud" => Ok(Self::Hud),
            _ => Err(format!(
                "unknown layer: {text} (use line, edit, color, or hud)"
            )),
        }
    }
}

impl FromStr for Depth {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "16" => Ok(Self::Basic),
            "256" => Ok(Self::Indexed),
            "true" => Ok(Self::True),
            _ => Err(format!(
                "unknown color: {text} (use none, 16, 256, or true)"
            )),
        }
    }
}

impl Choices {
    pub fn read(text: &str) -> Result<Self, String> {
        let mut choices = Self::default();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = |message: String| format!("line {}: {message}", index + 1);
            let Some((key, value)) = line.split_once('=') else {
                return Err(at(format!("cannot read this line: {line}")));
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "layer" => choices.layer = Some(value.parse().map_err(at)?),
                "color" => choices.color = Some(value.parse().map_err(at)?),
                "ascii" => choices.ascii = Some(yes(key, value).map_err(at)?),
                "verbose" => choices.verbose = Some(yes(key, value).map_err(at)?),
                other => return Err(at(format!("unknown setting: {other}"))),
            }
        }
        Ok(choices)
    }

    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self {
            layer: self.layer.or(other.layer),
            color: self.color.or(other.color),
            ascii: self.ascii.or(other.ascii),
            verbose: self.verbose.or(other.verbose),
        }
    }

    #[must_use]
    pub fn settle(self, found: Settings) -> Settings {
        Settings {
            layer: self
                .layer
                .map_or(found.layer, |layer| layer.min(found.layer)),
            color: self.color.unwrap_or(found.color),
            ascii: self.ascii.unwrap_or(found.ascii),
            verbose: self.verbose.unwrap_or(found.verbose),
        }
    }
}

pub fn read_settings(dir: &Path) -> Result<Choices, String> {
    let path = dir.join(FILE);
    let place = path.display();
    match fs::read_to_string(&path) {
        Ok(text) => Choices::read(&text).map_err(|error| format!("{place}: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Choices::default()),
        Err(error) => Err(format!("cannot read {place}: {error}")),
    }
}

fn yes(key: &str, value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("bad {key}: {value} (use yes or no)")),
    }
}

#[must_use]
pub fn detect(var: impl Fn(&str) -> Option<String>, terminal: bool, windows: bool) -> Settings {
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    let term = set("TERM").unwrap_or_default().to_ascii_lowercase();
    let layer = if terminal && term != "dumb" {
        Layer::Hud
    } else {
        Layer::Line
    };
    let truecolor = set("COLORTERM").is_some_and(|value| value == "truecolor" || value == "24bit");
    let color = if set("NO_COLOR").is_some() {
        Depth::None
    } else if windows || truecolor || set("WT_SESSION").is_some() {
        Depth::True
    } else if term.contains("256color") {
        Depth::Indexed
    } else {
        Depth::Basic
    };
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(set)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let utf8 = windows || locale.contains("utf-8") || locale.contains("utf8");
    Settings {
        layer,
        color,
        ascii: !utf8,
        verbose: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Choices, Depth, Layer, Settings, detect, read_settings};
    use crate::testing::scratch;

    #[test]
    fn values_have_their_names() {
        assert_eq!("hud".parse::<Layer>(), Ok(Layer::Hud));
        assert_eq!("LINE".parse::<Layer>(), Ok(Layer::Line));
        assert_eq!("256".parse::<Depth>(), Ok(Depth::Indexed));
        assert_eq!("true".parse::<Depth>(), Ok(Depth::True));
        assert_eq!(
            "3d".parse::<Layer>(),
            Err("unknown layer: 3d (use line, edit, color, or hud)".to_owned())
        );
        assert_eq!(
            "lots".parse::<Depth>(),
            Err("unknown color: lots (use none, 16, 256, or true)".to_owned())
        );
    }

    #[test]
    fn a_settings_file_sets_the_choices() {
        let text = "# My settings.\nlayer = color\n\ncolor = 16\nascii = yes\nverbose = no\n";
        assert_eq!(
            Choices::read(text),
            Ok(Choices {
                layer: Some(Layer::Color),
                color: Some(Depth::Basic),
                ascii: Some(true),
                verbose: Some(false),
            })
        );
        assert_eq!(Choices::read(""), Ok(Choices::default()));
    }

    #[test]
    fn the_settings_file_is_in_the_config_directory() {
        let dir = scratch("settings");
        assert_eq!(read_settings(&dir), Ok(Choices::default()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.ini"), "ascii = yes\n").unwrap();
        let ascii = read_settings(&dir).map(|choices| choices.ascii);
        assert_eq!(ascii, Ok(Some(true)));
        std::fs::write(dir.join("settings.ini"), "ascii = maybe\n").unwrap();
        let error = read_settings(&dir).unwrap_err();
        assert!(
            error.ends_with("settings.ini: line 1: bad ascii: maybe (use yes or no)"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_settings_file_fails_with_the_line() {
        assert_eq!(
            Choices::read("layer = hud\nlayer = 3d\n"),
            Err("line 2: unknown layer: 3d (use line, edit, color, or hud)".to_owned())
        );
        assert_eq!(
            Choices::read("size = big\n"),
            Err("line 1: unknown setting: size".to_owned())
        );
        assert_eq!(
            Choices::read("ascii = maybe\n"),
            Err("line 1: bad ascii: maybe (use yes or no)".to_owned())
        );
        assert_eq!(
            Choices::read("ascii\n"),
            Err("line 1: cannot read this line: ascii".to_owned())
        );
    }

    #[test]
    fn options_win_over_the_file_and_the_terminal_sets_the_limit() {
        let file = Choices {
            layer: Some(Layer::Edit),
            color: Some(Depth::Basic),
            ..Choices::default()
        };
        let options = Choices {
            color: Some(Depth::True),
            verbose: Some(true),
            ..Choices::default()
        };
        let found = Settings {
            layer: Layer::Hud,
            color: Depth::Indexed,
            ascii: false,
            verbose: false,
        };
        assert_eq!(
            options.or(file).settle(found),
            Settings {
                layer: Layer::Edit,
                color: Depth::True,
                ascii: false,
                verbose: true,
            }
        );
        let wide = Choices {
            layer: Some(Layer::Hud),
            ..Choices::default()
        };
        let small = Settings {
            layer: Layer::Line,
            ..found
        };
        assert_eq!(wide.settle(small).layer, Layer::Line);
    }

    fn found(vars: &[(&str, &str)], terminal: bool, windows: bool) -> Settings {
        let var = |name: &str| {
            let pair = vars.iter().find(|(key, _)| *key == name);
            pair.map(|(_, value)| (*value).to_owned())
        };
        detect(var, terminal, windows)
    }

    #[test]
    fn a_pipe_or_a_dumb_terminal_gets_line_mode() {
        assert_eq!(found(&[("TERM", "xterm")], false, false).layer, Layer::Line);
        assert_eq!(found(&[("TERM", "dumb")], true, false).layer, Layer::Line);
        assert_eq!(found(&[("TERM", "xterm")], true, false).layer, Layer::Hud);
        assert_eq!(found(&[], true, true).layer, Layer::Hud);
    }

    #[test]
    fn the_color_depth_comes_from_the_environment() {
        let depth = |vars: &[(&str, &str)], windows| found(vars, true, windows).color;
        assert_eq!(depth(&[("TERM", "xterm")], false), Depth::Basic);
        assert_eq!(depth(&[("TERM", "xterm-256color")], false), Depth::Indexed);
        let truecolor = [("TERM", "xterm"), ("COLORTERM", "truecolor")];
        assert_eq!(depth(&truecolor, false), Depth::True);
        assert_eq!(depth(&[("WT_SESSION", "1f2e")], false), Depth::True);
        assert_eq!(
            depth(&[("TERM", "xterm"), ("NO_COLOR", "1")], false),
            Depth::None
        );
        assert_eq!(depth(&[("NO_COLOR", "")], true), Depth::True);
    }

    #[test]
    fn utf8_comes_from_the_locale_and_windows_always_has_it() {
        let ascii = |vars: &[(&str, &str)], windows| found(vars, true, windows).ascii;
        assert!(!ascii(&[("LANG", "en_US.UTF-8")], false));
        assert!(!ascii(&[("LC_CTYPE", "C.utf8")], false));
        assert!(ascii(&[("LANG", "C")], false));
        assert!(ascii(&[("LC_ALL", "C"), ("LANG", "en_US.UTF-8")], false));
        assert!(ascii(&[], false));
        assert!(!ascii(&[], true));
    }
}
