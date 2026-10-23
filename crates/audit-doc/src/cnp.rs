//! The CNP 2021 list (516 codes, from IRCC's « Trouver votre CNP » page)
//! and a search by job title, so the audit can propose the codes matching
//! what the client wrote.

use std::sync::OnceLock;

use form_import::norm;

/// Source: https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/trouver-classification-nationale-professions.html
const LIST: &str = include_str!("../data/cnp-2021.csv");

/// More matches than this: the title is too vague to propose codes.
const MAX_CHOICES: usize = 6;

/// Words that say nothing about the occupation.
const STOP: &[&str] = &[
    "des",
    "les",
    "aux",
    "pour",
    "avec",
    "dans",
    "sur",
    "par",
    "une",
    "autre",
    "autres",
    "personnel",
    "assimile",
    "assimiles",
    "general",
    "generale",
    "generaux",
    "chez",
    "son",
    "ses",
    "leur",
    "dont",
    "ans",
    "experience",
    "annee",
    "annees",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub code: &'static str,
    pub title: &'static str,
    words: Vec<String>,
}

fn entries() -> &'static [Entry] {
    static ALL: OnceLock<Vec<Entry>> = OnceLock::new();
    ALL.get_or_init(|| {
        LIST.lines()
            .skip(1)
            .filter_map(|line| {
                // FEER,code,title — the title may be quoted (it holds commas).
                let mut it = line.splitn(3, ',');
                let _feer = it.next()?;
                let code = it.next()?;
                let title = it.next()?.trim_matches('"');
                Some(Entry {
                    code,
                    title,
                    words: words(title),
                })
            })
            .collect()
    })
}

/// Significant words, without accents, case or plural (« Cuisinières » → « cuisiniere »).
fn words(text: &str) -> Vec<String> {
    norm(text)
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() >= 3 && !STOP.contains(w))
        .map(|w| {
            let w = if w.len() > 4 && (w.ends_with('s') || w.ends_with('x')) {
                &w[..w.len() - 1]
            } else {
                w
            };
            w.to_owned()
        })
        .collect()
}

/// The official title of a code, if it exists.
pub fn title(code: &str) -> Option<&'static str> {
    entries().iter().find(|e| e.code == code).map(|e| e.title)
}

/// The codes whose title best matches a job description, most specific
/// first, and whether every word of the description is in their title.
/// Empty when nothing matches or the description is too vague.
pub fn search(job: &str) -> (Vec<&'static Entry>, bool) {
    let wanted = words(job);
    if wanted.is_empty() {
        return (Vec::new(), false);
    }
    let scored: Vec<(usize, &Entry)> = entries()
        .iter()
        .map(|e| (wanted.iter().filter(|w| e.words.contains(w)).count(), e))
        .filter(|(n, _)| *n > 0)
        .collect();
    let Some(best) = scored.iter().map(|(n, _)| *n).max() else {
        return (Vec::new(), false);
    };
    let mut found: Vec<&Entry> = scored
        .into_iter()
        .filter(|(n, _)| *n == best)
        .map(|(_, e)| e)
        .collect();
    if found.len() > MAX_CHOICES {
        return (Vec::new(), false);
    }
    found.sort_by_key(|e| e.words.len());
    (found, best == wanted.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_list_is_loaded() {
        assert_eq!(entries().len(), 516);
        assert_eq!(title("63200"), Some("Cuisiniers/cuisinières"));
        assert_eq!(title("99999"), None);
        assert!(
            entries()
                .iter()
                .all(|e| e.code.len() == 5 && !e.title.is_empty())
        );
    }

    #[test]
    fn one_match_or_several_or_none() {
        let codes = |job: &str| search(job).0.iter().map(|e| e.code).collect::<Vec<_>>();
        assert_eq!(codes("Cuisinier"), ["63200"]);
        assert_eq!(codes("Infirmière praticienne"), ["31302"]);
        assert!(codes("Infirmière").len() > 1);
        assert!(codes("Infirmière").contains(&"31301"));
        assert!(codes("Technicien").is_empty(), "too vague");
        assert!(codes("").is_empty());
        assert!(codes("Astronaute lunaire").is_empty());
        assert!(search("Cuisinier").1);
        assert!(!search("Cuisinier en chef de brigade").1, "partial match");
    }
}
