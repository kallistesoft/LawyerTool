//! CRS grid from `criteries-scg.html`.

use std::sync::LazyLock;

use applicant::EducationLevel;
use regex::Regex;
use rules_engine::grid::{
    Additional, Cap, Crs, EduClass, Grid, IsoDate, Points, Range, Row, SingleRow, Source, TierRow,
    TierTable, Transferability,
};

use crate::ParseError;
use crate::page::{Page, Table};

fn re(p: &str) -> Regex {
    // Patterns are constants in this file; a bad one is a bug.
    #[allow(clippy::expect_used)]
    Regex::new(p).expect("valid regex")
}

static MAXIMUM: LazyLock<Regex> = LazyLock::new(|| re(r"Maximum (\d+) points"));
static NCLC_MIN: LazyLock<Regex> = LazyLock::new(|| re(r"NCLC (\d+) ou plus"));
static CDN_ONE: LazyLock<Regex> =
    LazyLock::new(|| re(r"\+ (\w+) années? d'expérience de travail au Canada"));
static CDN_MORE: LazyLock<Regex> =
    LazyLock::new(|| re(r"\+ (\w+) années? ou plus d'expérience de travail au Canada"));

const EDUCATION: &[(&str, EducationLevel)] = &[
    (
        r"^Études secondaires non complétées",
        EducationLevel::LessThanSecondary,
    ),
    (r"^Diplôme d'études secondaires", EducationLevel::Secondary),
    (
        r"^Degré, diplôme ou certificat de un an",
        EducationLevel::OneYearPostSecondary,
    ),
    (
        r"^Degré, diplôme ou certificat de deux ans",
        EducationLevel::TwoYearPostSecondary,
    ),
    (
        r"^Baccalauréat OU autres programmes",
        EducationLevel::BachelorOrThreeYear,
    ),
    (
        r"^Au moins deux certificats",
        EducationLevel::TwoOrMoreCredentials,
    ),
    (
        r"^Diplôme d'études universitaires de deuxième cycle",
        EducationLevel::MasterOrProfessional,
    ),
    (
        r"^Diplôme universitaire de troisième cycle",
        EducationLevel::Doctorate,
    ),
];

const EDU_CLASS: &[(&str, EduClass)] = &[
    (
        r"^Diplôme d'école secondaire ou moins$",
        EduClass::SecondaryOrLess,
    ),
    (
        r"^Diplôme postsecondaire nécessitant au moins une année d'études$",
        EduClass::PostSecondary,
    ),
    (
        r"^Deux diplômes postsecondaires ou plus ET",
        EduClass::TwoOrMore,
    ),
    (
        r"^il a obtenu un diplôme d'études universitaires de deuxième cycle",
        EduClass::MasterOrProfessional,
    ),
    (
        r"^il a obtenu un diplôme d'études universitaires de troisième cycle$",
        EduClass::Doctorate,
    ),
];

#[derive(Clone, Copy)]
enum Extra {
    Sibling,
    FrenchWeak,
    FrenchStrong,
    CdnShort,
    CdnLong,
    Nomination,
}

const ADDITIONAL: &[(&str, Extra)] = &[
    (r"^Frère ou sœur vivant au Canada", Extra::Sibling),
    (
        r"^Si vous obtenez le niveau (\d+) ou plus des NCLC en français pour les quatre aptitudes et si vous obtenez le niveau (\d+) ou moins des CLB en anglais",
        Extra::FrenchWeak,
    ),
    (
        r"^Si vous obtenez le niveau (\d+) ou plus des NCLC en français pour les quatre aptitudes et si vous obtenez le niveau (\d+) ou plus des CLB en anglais",
        Extra::FrenchStrong,
    ),
    (
        r"^Études postsecondaires effectuées au Canada – titre de compétence d'un an ou de deux ans$",
        Extra::CdnShort,
    ),
    (
        r"^Études postsecondaires effectuées au Canada – titre de compétence de trois ans ou plus$",
        Extra::CdnLong,
    ),
    (
        r"^Désignation comme candidat d'une province ou d'un territoire$",
        Extra::Nomination,
    ),
];

/// Parses the CRS grid. `page.source` must equal `source.id`.
pub fn parse_crs(
    page: &Page,
    id: &str,
    effective_from: IsoDate,
    source: Source,
) -> Result<Grid, ParseError> {
    let p = Parser { page };
    let crs = Crs {
        core_max: p.core_max()?,
        age: p.age()?,
        education: p.education()?,
        first_language: p.first_language()?,
        second_language: p.second_language()?.0,
        second_language_max: p.second_language()?.1,
        canadian_work: p.canadian_work()?,
        spouse_max: p.spouse_max()?,
        spouse_education: p.spouse_education()?,
        spouse_language: p.spouse_language()?,
        spouse_canadian_work: p.spouse_canadian_work()?,
        transferability: p.transferability()?,
        additional: p.additional()?,
    };
    Ok(Grid {
        id: id.to_owned(),
        effective_from,
        sources: vec![source],
        crs,
    })
}

struct Parser<'a> {
    page: &'a Page,
}

impl Parser<'_> {
    fn src(&self) -> &str {
        &self.page.source
    }

    fn core_max(&self) -> Result<Row<()>, ParseError> {
        let r = re(
            r"Avec un époux ou conjoint de fait ?: ?Maximum (\d+) points total pour tous les facteurs Sans époux ou conjoint de fait ?: ?Maximum (\d+) points",
        );
        let c = r
            .captures(&self.page.text)
            .ok_or(ParseError::TextNotFound("core maximum"))?;
        Ok(Row {
            key: (),
            points: Points {
                with_spouse: cap_num(&c, 1)?,
                without_spouse: cap_num(&c, 2)?,
            },
            cite: self.page.text_cite(&c[0]),
        })
    }

    fn age(&self) -> Result<Vec<Row<Range>>, ParseError> {
        let what = "age";
        let t = self.page.table(what, |t| t.h(0) == "Âge")?;
        t.expect_cols(what, 3)?;
        ranged(t, what, parse_age)?
            .into_iter()
            .map(|(row, key)| self.row(t, what, row, key))
            .collect()
    }

    fn education(&self) -> Result<Vec<Row<EducationLevel>>, ParseError> {
        let what = "education";
        let t = self.page.table(what, |t| {
            t.h(0) == "Niveau de scolarité" && t.h(1).starts_with("Avec un époux")
        })?;
        t.expect_cols(what, 3)?;
        labelled(t, what, EDUCATION)?
            .into_iter()
            .map(|(row, key, _)| self.row(t, what, row, key))
            .collect()
    }

    fn language_table(&self, what: &'static str, heading: &str) -> Result<&Table, ParseError> {
        let t = self.page.table(what, |t| {
            t.heading.contains(heading)
                && !t.heading.contains("époux")
                && t.h(0)
                    .starts_with("Niveaux de compétence linguistique canadiens")
        })?;
        t.expect_cols(what, 3)?;
        Ok(t)
    }

    fn first_language(&self) -> Result<Vec<Row<Range>>, ParseError> {
        let what = "first official language";
        let t = self.language_table(what, "première langue officielle")?;
        ranged(t, what, parse_clb)?
            .into_iter()
            .map(|(row, key)| self.row(t, what, row, key))
            .collect()
    }

    fn second_language(&self) -> Result<(Vec<Row<Range>>, Row<()>), ParseError> {
        let what = "second official language";
        let t = self.language_table(what, "deuxième langue officielle")?;
        let rows = ranged(t, what, parse_clb)?
            .into_iter()
            .map(|(row, key)| self.row(t, what, row, key))
            .collect::<Result<_, _>>()?;
        let max = Row {
            key: (),
            points: Points {
                with_spouse: maximum(t, what, 1)?,
                without_spouse: maximum(t, what, 2)?,
            },
            cite: t.cite(self.src(), 0),
        };
        Ok((rows, max))
    }

    fn canadian_work(&self) -> Result<Vec<Row<Range>>, ParseError> {
        let what = "Canadian work experience";
        let t = self.page.table(what, |t| {
            t.h(0) == "Expérience de travail au Canada" && t.header().len() == 3
        })?;
        t.expect_cols(what, 3)?;
        ranged(t, what, parse_years)?
            .into_iter()
            .map(|(row, key)| self.row(t, what, row, key))
            .collect()
    }

    fn spouse_max(&self) -> Result<Cap, ParseError> {
        let what = "spouse factors maximum";
        let t = self.page.table(what, |t| {
            t.heading.starts_with("B. Facteurs liés à l'époux") && t.h(0) == "Facteur"
        })?;
        Ok(Cap {
            value: maximum(t, what, 1)?,
            cite: t.cite(self.src(), 0),
        })
    }

    fn spouse_education(&self) -> Result<Vec<SingleRow<EducationLevel>>, ParseError> {
        let what = "spouse education";
        let t = self.page.table(what, |t| {
            t.h(0).starts_with("Niveau de scolarité de l'époux")
        })?;
        t.expect_cols(what, 3)?;
        labelled(t, what, EDUCATION)?
            .into_iter()
            .map(|(row, key, _)| self.spouse_row(t, what, row, key))
            .collect()
    }

    fn spouse_language(&self) -> Result<Vec<SingleRow<Range>>, ParseError> {
        let what = "spouse language";
        let t = self.page.table(what, |t| {
            t.heading.contains("époux")
                && t.h(0)
                    .starts_with("Niveaux de compétence linguistique canadiens")
        })?;
        t.expect_cols(what, 3)?;
        ranged(t, what, parse_clb)?
            .into_iter()
            .map(|(row, key)| self.spouse_row(t, what, row, key))
            .collect()
    }

    fn spouse_canadian_work(&self) -> Result<Vec<SingleRow<Range>>, ParseError> {
        let what = "spouse Canadian work experience";
        let t = self.page.table(what, |t| {
            t.h(0) == "Expérience de travail au Canada du conjoint"
        })?;
        t.expect_cols(what, 3)?;
        ranged(t, what, parse_years)?
            .into_iter()
            .map(|(row, key)| self.spouse_row(t, what, row, key))
            .collect()
    }

    fn transferability(&self) -> Result<Transferability, ParseError> {
        let page = self.page;

        let what = "transferability: education + language";
        let t = page.table(what, |t| {
            t.heading == "Études"
                && t.h(0)
                    .starts_with("Avec une bonne maîtrise d'une langue officielle")
        })?;
        let education_language = self.tier_table(t, what, &NCLC_MIN, &NCLC_MIN, |t, w| {
            labelled(t, w, EDU_CLASS).map(|v| v.into_iter().map(|(r, k, _)| (r, k)).collect())
        })?;

        let what = "transferability: education + Canadian work";
        let t = page.table(what, |t| {
            t.h(0).starts_with(
                "Avec une expérience de travail au Canada et un diplôme postsecondaire",
            )
        })?;
        let education_canadian_work = self.tier_table(t, what, &CDN_ONE, &CDN_MORE, |t, w| {
            labelled(t, w, EDU_CLASS).map(|v| v.into_iter().map(|(r, k, _)| (r, k)).collect())
        })?;

        let what = "transferability: foreign work + language";
        let t = page.table(what, |t| {
            t.heading
                .starts_with("Expérience de travail à l'étranger – Avec une bonne maîtrise")
                && t.h(0) == "Années d'expérience"
        })?;
        let foreign_language = self.tier_table(t, what, &NCLC_MIN, &NCLC_MIN, |t, w| {
            ranged(t, w, parse_foreign)
        })?;

        let what = "transferability: foreign work + Canadian work";
        let t = page.table(what, |t| {
            t.heading
                .starts_with("Avec une expérience de travail AU Canada et une expérience")
                && t.h(0) == "Années d'expérience"
        })?;
        let foreign_canadian_work = self.tier_table(t, what, &CDN_ONE, &CDN_MORE, |t, w| {
            ranged(t, w, parse_foreign)
        })?;

        let what = "transferability: certificate of qualification";
        let t = page.table(what, |t| {
            t.h(0)
                .starts_with("Certificat de compétence (métier spécialisé)")
        })?;
        let certificate = self.tier_table(t, what, &NCLC_MIN, &NCLC_MIN, |t, w| {
            labelled(t, w, &[(r"^Avec un certificat de compétence$", ())])
                .map(|v| v.into_iter().map(|(r, k, _)| (r, k)).collect())
        })?;

        // The three group-cap tables must agree.
        let what = "transferability group maximum";
        let groups = [
            page.table(what, |t| t.h(0) == "Scolarité")?,
            page.table(what, |t| t.h(0) == "Expérience de travail à l'étranger")?,
            page.table(what, |t| {
                t.h(0).starts_with("Certificat de compétence (pour")
            })?,
        ];
        let group_max = maximum(groups[0], what, 1)?;
        for g in &groups[1..] {
            if maximum(g, what, 1)? != group_max {
                return Err(ParseError::BadNumber {
                    what,
                    table: g.index,
                    row: 0,
                    text: g.h(1).to_owned(),
                });
            }
        }

        let r = re(r"transférabilité des compétences ?: ?Maximum (\d+) points");
        let c = r
            .captures(&page.text)
            .ok_or(ParseError::TextNotFound("transferability maximum"))?;

        Ok(Transferability {
            education_language,
            education_canadian_work,
            foreign_language,
            foreign_canadian_work,
            certificate,
            group_max: Cap {
                value: group_max,
                cite: groups[0].cite(self.src(), 0),
            },
            max: Cap {
                value: cap_num(&c, 1)?,
                cite: page.text_cite(&c[0]),
            },
        })
    }

    fn additional(&self) -> Result<Additional, ParseError> {
        let what = "additional points";
        let t = self
            .page
            .table(what, |t| t.h(0) == "Points supplémentaires")?;
        t.expect_cols(what, 2)?;
        let max = Cap {
            value: maximum(t, what, 1)?,
            cite: t.cite(self.src(), 0),
        };

        let rows = labelled(t, what, ADDITIONAL)?;
        let get = |want: Extra| {
            rows.iter()
                .find(|(_, k, _)| std::mem::discriminant(k) == std::mem::discriminant(&want))
                .map(|(row, _, caps)| (*row, caps.clone()))
                .ok_or(ParseError::MissingRow {
                    what,
                    label: "additional points row",
                })
        };
        let single = |row: u16| -> Result<SingleRow<()>, ParseError> {
            Ok(SingleRow {
                key: (),
                points: cell_num(t, what, row, 1)?,
                cite: t.cite(self.src(), row),
            })
        };

        let (weak_row, weak) = get(Extra::FrenchWeak)?;
        let (strong_row, strong) = get(Extra::FrenchStrong)?;
        let bad = |row: u16| ParseError::BadLabel {
            what,
            table: t.index,
            row,
            text: t.rows[usize::from(row)][0].clone(),
        };
        let num = |s: &str, row: u16| s.parse::<u8>().map_err(|_| bad(row));
        let french_min_nclc = num(&strong[0], strong_row)?;
        let english_min_clb = num(&strong[1], strong_row)?;
        // Weak row: same French threshold, English "N or less" with N = min-1.
        if num(&weak[0], weak_row)? != french_min_nclc
            || num(&weak[1], weak_row)?.checked_add(1) != Some(english_min_clb)
        {
            return Err(bad(weak_row));
        }

        Ok(Additional {
            sibling: single(get(Extra::Sibling)?.0)?,
            french_weak_english: single(weak_row)?,
            french_strong_english: single(strong_row)?,
            french_min_nclc,
            english_min_clb,
            canadian_education_short: single(get(Extra::CdnShort)?.0)?,
            canadian_education_long: single(get(Extra::CdnLong)?.0)?,
            provincial_nomination: single(get(Extra::Nomination)?.0)?,
            max,
        })
    }

    fn row<K>(
        &self,
        t: &Table,
        what: &'static str,
        row: u16,
        key: K,
    ) -> Result<Row<K>, ParseError> {
        Ok(Row {
            key,
            points: Points {
                with_spouse: cell_num(t, what, row, 1)?,
                without_spouse: cell_num(t, what, row, 2)?,
            },
            cite: t.cite(self.src(), row),
        })
    }

    /// Spouse tables: points in column 1, "n/a" required in column 2.
    fn spouse_row<K>(
        &self,
        t: &Table,
        what: &'static str,
        row: u16,
        key: K,
    ) -> Result<SingleRow<K>, ParseError> {
        let na = &t.rows[usize::from(row)][2];
        if na != "n/a" {
            return Err(ParseError::BadNumber {
                what,
                table: t.index,
                row,
                text: na.clone(),
            });
        }
        Ok(SingleRow {
            key,
            points: cell_num(t, what, row, 1)?,
            cite: t.cite(self.src(), row),
        })
    }

    fn tier_table<K>(
        &self,
        t: &Table,
        what: &'static str,
        lower_re: &Regex,
        upper_re: &Regex,
        keys: impl Fn(&Table, &'static str) -> Result<Vec<(u16, K)>, ParseError>,
    ) -> Result<TierTable<K>, ParseError> {
        t.expect_cols(what, 3)?;
        let threshold = |col: usize, r: &Regex| -> Result<u8, ParseError> {
            r.captures(t.h(col))
                .and_then(|c| level(&c[1]))
                .ok_or_else(|| ParseError::BadLabel {
                    what,
                    table: t.index,
                    row: 0,
                    text: t.h(col).to_owned(),
                })
        };
        let rows = keys(t, what)?
            .into_iter()
            .map(|(row, key)| {
                Ok(TierRow {
                    key,
                    lower: cell_num(t, what, row, 1)?,
                    upper: cell_num(t, what, row, 2)?,
                    cite: t.cite(self.src(), row),
                })
            })
            .collect::<Result<_, ParseError>>()?;
        Ok(TierTable {
            lower_min: threshold(1, lower_re)?,
            upper_min: threshold(2, upper_re)?,
            rows,
            cite: t.cite(self.src(), 0),
        })
    }
}

/// Matches every body row against exactly one pattern; every pattern must be
/// used exactly once. Returns (row, key, captures).
fn labelled<K: Copy>(
    t: &Table,
    what: &'static str,
    labels: &[(&'static str, K)],
) -> Result<Vec<(u16, K, Vec<String>)>, ParseError> {
    let patterns: Vec<Regex> = labels.iter().map(|(p, _)| re(p)).collect();
    let mut used = vec![false; labels.len()];
    let mut out = Vec::new();
    for (row, cells) in t.body() {
        let label = cells.first().map(String::as_str).unwrap_or("");
        let hits: Vec<usize> = (0..labels.len())
            .filter(|&i| patterns[i].is_match(label))
            .collect();
        let unexpected = || ParseError::UnexpectedRow {
            what,
            table: t.index,
            row,
            text: label.to_owned(),
        };
        let [i] = hits[..] else {
            return Err(unexpected());
        };
        if std::mem::replace(&mut used[i], true) {
            return Err(unexpected());
        }
        let caps = patterns[i]
            .captures(label)
            .map(|c| {
                c.iter()
                    .skip(1)
                    .flatten()
                    .map(|m| m.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        out.push((row, labels[i].1, caps));
    }
    match used.iter().position(|u| !u) {
        Some(i) => Err(ParseError::MissingRow {
            what,
            label: labels[i].0,
        }),
        None => Ok(out),
    }
}

fn ranged(
    t: &Table,
    what: &'static str,
    f: fn(&str) -> Option<Range>,
) -> Result<Vec<(u16, Range)>, ParseError> {
    t.body()
        .map(|(row, cells)| {
            let label = cells.first().map(String::as_str).unwrap_or("");
            f(label)
                .map(|r| (row, r))
                .ok_or_else(|| ParseError::BadLabel {
                    what,
                    table: t.index,
                    row,
                    text: label.to_owned(),
                })
        })
        .collect()
}

fn cell_num(t: &Table, what: &'static str, row: u16, col: usize) -> Result<u16, ParseError> {
    let text = t
        .rows
        .get(usize::from(row))
        .and_then(|r| r.get(col))
        .map(String::as_str)
        .unwrap_or("");
    let ok = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| text.parse().ok())
        .flatten()
        .ok_or_else(|| ParseError::BadNumber {
            what,
            table: t.index,
            row,
            text: text.to_owned(),
        })
}

fn maximum(t: &Table, what: &'static str, col: usize) -> Result<u16, ParseError> {
    MAXIMUM
        .captures(t.h(col))
        .and_then(|c| c[1].parse().ok())
        .ok_or_else(|| ParseError::BadNumber {
            what,
            table: t.index,
            row: 0,
            text: t.h(col).to_owned(),
        })
}

fn cap_num(c: &regex::Captures, i: usize) -> Result<u16, ParseError> {
    c[i].parse().map_err(|_| ParseError::TextNotFound("number"))
}

/// "7" or a French number word.
fn level(s: &str) -> Option<u8> {
    match s.to_lowercase().as_str() {
        "un" | "une" => Some(1),
        "deux" => Some(2),
        "trois" => Some(3),
        "quatre" => Some(4),
        "cinq" => Some(5),
        n => n.parse().ok(),
    }
}

fn caps(r: &str, s: &str) -> Option<Vec<u8>> {
    let c = re(r).captures(s)?;
    c.iter()
        .skip(1)
        .map(|m| m.and_then(|m| level(m.as_str())))
        .collect()
}

fn parse_age(s: &str) -> Option<Range> {
    if let Some(v) = caps(r"^(\d+) ans ou moins$", s) {
        return Some(Range {
            min: 0,
            max: Some(v[0]),
        });
    }
    if let Some(v) = caps(r"^(\d+) ans ou plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    if let Some(v) = caps(r"^(\d+) (?:to|à) (\d+) ans$", s) {
        return Some(Range {
            min: v[0],
            max: Some(v[1]),
        });
    }
    caps(r"^(\d+) ans$", s).map(|v| Range {
        min: v[0],
        max: Some(v[0]),
    })
}

fn parse_clb(s: &str) -> Option<Range> {
    if let Some(v) = caps(r"^Moins que le niveau (\d+) des NCLC$", s) {
        return Some(Range {
            min: 0,
            max: Some(v[0].checked_sub(1)?),
        });
    }
    if let Some(v) = caps(r"^NCLC (\d+) ou moins$", s) {
        return Some(Range {
            min: 0,
            max: Some(v[0]),
        });
    }
    if let Some(v) = caps(r"^NCLC (\d+) ou plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    if let Some(v) = caps(r"^NCLC (\d+) ou (\d+)$", s) {
        return Some(Range {
            min: v[0],
            max: Some(v[1]),
        });
    }
    caps(r"^NCLC (\d+)$", s).map(|v| Range {
        min: v[0],
        max: Some(v[0]),
    })
}

fn parse_years(s: &str) -> Option<Range> {
    if s == "Aucune ou moins d'un an" {
        return Some(Range {
            min: 0,
            max: Some(0),
        });
    }
    if let Some(v) = caps(r"^(\d+) ans? ou plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    caps(r"^(\d+) ans?$", s).map(|v| Range {
        min: v[0],
        max: Some(v[0]),
    })
}

fn parse_foreign(s: &str) -> Option<Range> {
    const TAIL: &str = r" d'expérience de travail à l'étranger$";
    if s == "Aucune expérience de travail à l'étranger" {
        return Some(Range {
            min: 0,
            max: Some(0),
        });
    }
    if let Some(v) = caps(&format!(r"^(\w+) années? ou plus{TAIL}"), s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    caps(&format!(r"^(\w+) années? ou (\w+) années?{TAIL}"), s).map(|v| Range {
        min: v[0],
        max: Some(v[1]),
    })
}
