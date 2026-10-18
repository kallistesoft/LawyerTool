//! FSW / CEC / FST requirements and proof of funds from the program pages.

use applicant::EducationLevel;
use rules_engine::grid::{Cap, Range, SingleRow};
use rules_engine::programs::{
    Adaptability, Cec, ExperienceRule, Fst, Fsw, Funds, FundsRow, NocGroup, Programs,
};

use crate::ParseError;
use crate::crs::{cell_num, labelled, ranged, re};
use crate::page::{Page, Table};

/// The saved pages, by role.
pub struct ProgramPages<'a> {
    pub fsw: &'a Page,
    pub cec: &'a Page,
    pub fst: &'a Page,
    pub language: &'a Page,
    pub education: &'a Page,
    pub funds: &'a Page,
}

/// A number captured from the running text, e.g. "1 560 heures".
fn text_num(page: &Page, what: &'static str, pattern: &str) -> Result<Cap, ParseError> {
    let c = re(pattern)
        .captures(&page.text)
        .ok_or(ParseError::TextNotFound(what))?;
    let digits: String = c.iter().skip(1).flatten().map(|m| m.as_str()).collect();
    let value = digits.parse().map_err(|_| ParseError::TextNotFound(what))?;
    Ok(Cap {
        value,
        cite: page.text_cite(&c[0]),
    })
}

fn single_rows<K>(
    page: &Page,
    t: &Table,
    what: &'static str,
    keys: Vec<(u16, K)>,
    col: usize,
) -> Result<Vec<SingleRow<K>>, ParseError> {
    keys.into_iter()
        .map(|(row, key)| {
            Ok(SingleRow {
                key,
                points: cell_num(t, what, row, col)?,
                cite: t.cite(&page.source, row),
            })
        })
        .collect()
}

fn caps(pattern: &str, s: &str) -> Option<Vec<u8>> {
    let c = re(pattern).captures(s)?;
    c.iter()
        .skip(1)
        .map(|m| m.and_then(|m| m.as_str().parse().ok()))
        .collect()
}

fn fsw_clb(s: &str) -> Option<Range> {
    if let Some(v) = caps(r"^Niveau (\d+) des NCLC ou plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    if let Some(v) = caps(r"^Moins que le niveau (\d+) des NCLC$", s) {
        return Some(Range {
            min: 0,
            max: Some(v[0].checked_sub(1)?),
        });
    }
    caps(r"^Niveau (\d+) des NCLC$", s).map(|v| Range {
        min: v[0],
        max: Some(v[0]),
    })
}

fn fsw_years(s: &str) -> Option<Range> {
    if let Some(v) = caps(r"^(\d+) ans ou plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    if let Some(v) = caps(r"^(\d+) ou (\d+) ans$", s) {
        return Some(Range {
            min: v[0],
            max: Some(v[1]),
        });
    }
    caps(r"^(\d+) ans?$", s).map(|v| Range {
        min: v[0],
        max: Some(v[0]),
    })
}

fn fsw_age(s: &str) -> Option<Range> {
    if let Some(v) = caps(r"^Moins de (\d+) ans$", s) {
        return Some(Range {
            min: 0,
            max: Some(v[0].checked_sub(1)?),
        });
    }
    if let Some(v) = caps(r"^(\d+) ans et plus$", s) {
        return Some(Range {
            min: v[0],
            max: None,
        });
    }
    if let Some(v) = caps(r"^(\d+)-(\d+) ans$", s) {
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

/// "CLB 7", "NCLC 7" or "NCLC7".
fn clb_cell(s: &str) -> Option<u8> {
    caps(r"^(?:CLB|NCLC) ?(\d+)$", s).map(|v| v[0])
}

fn fsw(p: &ProgramPages) -> Result<Fsw, ParseError> {
    let page = p.fsw;
    let src = &page.source;

    let what = "FSW first official language";
    let t = page.table(what, |t| t.h(0) == "Première langue officielle")?;
    t.expect_cols(what, 5)?;
    let mut first_language = Vec::new();
    let mut ineligible_below = None;
    for (row, range) in ranged(t, what, fsw_clb)? {
        let cells = &t.rows[usize::from(row)][1..];
        if cells.iter().all(|c| c == "Non admissible") {
            ineligible_below = range.max.map(|m| m + 1);
            continue;
        }
        let points = cell_num(t, what, row, 1)?;
        for col in 2..5 {
            if cell_num(t, what, row, col)? != points {
                return Err(ParseError::Inconsistent {
                    what,
                    detail: format!("row {row}: skills differ"),
                });
            }
        }
        first_language.push(SingleRow {
            key: range,
            points,
            cite: t.cite(src, row),
        });
    }

    let what = "FSW second official language";
    let t = page.table(what, |t| t.h(0) == "Seconde langue officielle")?;
    t.expect_cols(what, 2)?;
    let rows = labelled(
        t,
        what,
        &[
            (
                r"^Au moins le niveau (\d+) des NCLC pour les 4 compétences$",
                true,
            ),
            (
                r"^Niveau (\d+) des NCLC ou moins pour l'une des 4 compétences$",
                false,
            ),
        ],
    )?;
    let (yes_row, _, yes_caps) =
        rows.iter()
            .find(|r| r.1)
            .cloned()
            .ok_or(ParseError::MissingRow {
                what,
                label: "at least",
            })?;
    let (no_row, _, no_caps) =
        rows.iter()
            .find(|r| !r.1)
            .cloned()
            .ok_or(ParseError::MissingRow {
                what,
                label: "or less",
            })?;
    let second_language_min: u8 = yes_caps[0]
        .parse()
        .map_err(|_| ParseError::TextNotFound(what))?;
    if no_caps[0].parse::<u8>().ok().and_then(|n| n.checked_add(1)) != Some(second_language_min)
        || cell_num(t, what, no_row, 1)? != 0
    {
        return Err(ParseError::Inconsistent {
            what,
            detail: "thresholds".into(),
        });
    }
    let second_language = SingleRow {
        key: (),
        points: cell_num(t, what, yes_row, 1)?,
        cite: t.cite(src, yes_row),
    };

    let what = "FSW work experience";
    let t = page.table(what, |t| t.h(0) == "Expérience")?;
    t.expect_cols(what, 2)?;
    let work_experience = single_rows(page, t, what, ranged(t, what, fsw_years)?, 1)?;

    let what = "FSW age";
    let t = page.table(what, |t| t.h(0) == "Âge")?;
    t.expect_cols(what, 2)?;
    let age = single_rows(page, t, what, ranged(t, what, fsw_age)?, 1)?;

    let what = "FSW adaptability";
    let t = page.table(what, |t| t.h(0) == "Faculté d'adaptation")?;
    t.expect_cols(what, 2)?;
    #[derive(Clone, Copy, PartialEq)]
    enum A {
        SpouseLang,
        OwnStudy,
        SpouseStudy,
        OwnWork,
        SpouseWork,
        Arranged,
        Relative,
    }
    let rows = labelled(
        t,
        what,
        &[
            (
                r"^Niveau de compétence linguistique de votre époux ou conjoint de fait .*de (\d+) ou plus des NCLC pour les 4 compétences",
                A::SpouseLang,
            ),
            (r"^Vos études antérieures au Canada ", A::OwnStudy),
            (
                r"^Les études antérieures au Canada de votre époux ou conjoint de fait ",
                A::SpouseStudy,
            ),
            (r"^Votre travail antérieur au Canada ", A::OwnWork),
            (
                r"^Le travail antérieur de votre époux ou conjoint de fait au Canada ",
                A::SpouseWork,
            ),
            (r"^Emploi réservé au Canada ", A::Arranged),
            (r"^Membres de la famille au Canada ", A::Relative),
        ],
    )?;
    let get = |k: A| -> Result<(SingleRow<()>, Vec<String>), ParseError> {
        let (row, _, caps) = rows
            .iter()
            .find(|r| r.1 == k)
            .ok_or(ParseError::MissingRow {
                what,
                label: "adaptability row",
            })?;
        Ok((
            SingleRow {
                key: (),
                points: cell_num(t, what, *row, 1)?,
                cite: t.cite(src, *row),
            },
            caps.clone(),
        ))
    };
    let (spouse_language, sl_caps) = get(A::SpouseLang)?;
    let adaptability = Adaptability {
        spouse_language,
        spouse_language_min: sl_caps[0]
            .parse()
            .map_err(|_| ParseError::TextNotFound(what))?,
        own_canadian_study: get(A::OwnStudy)?.0,
        spouse_canadian_study: get(A::SpouseStudy)?.0,
        own_canadian_work: get(A::OwnWork)?.0,
        spouse_canadian_work: get(A::SpouseWork)?.0,
        arranged_employment: get(A::Arranged)?.0,
        relative_in_canada: get(A::Relative)?.0,
        max: Cap {
            value: re(r"^Maximum de (\d+) points$")
                .captures(t.h(1))
                .and_then(|c| c[1].parse().ok())
                .ok_or_else(|| ParseError::BadLabel {
                    what,
                    table: t.index,
                    row: 0,
                    text: t.h(1).to_owned(),
                })?,
            cite: t.cite(src, 0),
        },
    };

    let experience = ExperienceRule {
        min_hours: text_num(
            page,
            "FSW minimum hours",
            r"au moins une année de travail ou (\d) (\d{3}) heures au total",
        )?,
        window_years: text_num(
            page,
            "FSW experience window",
            r"avoir été acquise au cours des (\d+) dernières années",
        )?,
        max_hours_per_week: text_num(
            page,
            "FSW weekly cap",
            r"au-delà des (\d+) heures par semaine",
        )?,
    };

    // Minimum CLB: the language page, cross-checked with the "Non admissible" row.
    let min_clb = language_minimum(p.language, "FSW minimum language", |t| {
        t.h(0) == "Langue" && t.h(1) == "Niveau minimum pour les 4 compétences linguistiques"
    })?;
    if ineligible_below != Some(min_clb.value as u8) {
        return Err(ParseError::Inconsistent {
            what: "FSW minimum language",
            detail: format!(
                "language page {} vs FSW page {ineligible_below:?}",
                min_clb.value
            ),
        });
    }

    Ok(Fsw {
        experience,
        min_clb,
        first_language,
        second_language,
        second_language_min,
        education: fsw_education(p.education)?,
        work_experience,
        age,
        arranged_employment: text_num(
            page,
            "FSW arranged employment",
            r"Emploi réservé au Canada \(maximum de (\d+) points\)",
        )?,
        adaptability,
        pass_mark: text_num(
            page,
            "FSW pass mark",
            r"Si vous obtenez (\d+) points ou plus",
        )?,
    })
}

/// Two-column (language, level) table: English and French minimums must agree.
fn language_minimum(
    page: &Page,
    what: &'static str,
    pred: impl Fn(&Table) -> bool,
) -> Result<Cap, ParseError> {
    let t = page.table(what, pred)?;
    t.expect_cols(what, 2)?;
    let mut levels = Vec::new();
    for (row, cells) in t.body() {
        let lvl = clb_cell(&cells[1]).ok_or_else(|| ParseError::BadNumber {
            what,
            table: t.index,
            row,
            text: cells[1].clone(),
        })?;
        levels.push((cells[0].clone(), lvl));
    }
    match &levels[..] {
        [(a, x), (b, y)] if a == "Anglais" && b == "Français" && x == y => Ok(Cap {
            value: u16::from(*x),
            cite: t.cite(&page.source, 1),
        }),
        _ => Err(ParseError::Inconsistent {
            what,
            detail: format!("{levels:?}"),
        }),
    }
}

fn fsw_education(page: &Page) -> Result<Vec<SingleRow<EducationLevel>>, ParseError> {
    let what = "FSW education";
    let t = page.table(what, |t| {
        t.h(2) == "Points du facteur du Programme des travailleurs qualifiés (fédéral)"
    })?;
    t.expect_cols(what, 3)?;
    const LEVELS: &[(&str, EducationLevel)] = &[
        (
            "École secondaire (diplôme d'études secondaires)",
            EducationLevel::Secondary,
        ),
        (
            "Degré, diplôme ou certificat de un an",
            EducationLevel::OneYearPostSecondary,
        ),
        (
            "Degré, diplôme ou certificat de deux ans",
            EducationLevel::TwoYearPostSecondary,
        ),
        (
            "Baccalauréat ou autres programmes (trois ans ou plus)",
            EducationLevel::BachelorOrThreeYear,
        ),
        (
            "Au moins deux certificats, diplômes ou degrés",
            EducationLevel::TwoOrMoreCredentials,
        ),
        (
            "Diplôme universitaire au niveau de la maîtrise",
            EducationLevel::MasterOrProfessional,
        ),
        (
            "Diplôme professionnel requis pour la pratique",
            EducationLevel::MasterOrProfessional,
        ),
        (
            "Diplôme universitaire au niveau du doctorat",
            EducationLevel::Doctorate,
        ),
    ];
    let mut out: Vec<SingleRow<EducationLevel>> = Vec::new();
    for (row, cells) in t.body() {
        let level = LEVELS
            .iter()
            .find(|(p, _)| cells[1].starts_with(p))
            .map(|(_, l)| *l)
            .ok_or_else(|| ParseError::UnexpectedRow {
                what,
                table: t.index,
                row,
                text: cells[1].clone(),
            })?;
        let points = cell_num(t, what, row, 2)?;
        match out.iter().find(|r| r.key == level) {
            Some(r) if r.points != points => {
                return Err(ParseError::Inconsistent {
                    what,
                    detail: format!("{level:?}: {} vs {points} (row {row})", r.points),
                });
            }
            Some(_) => {}
            None => out.push(SingleRow {
                key: level,
                points,
                cite: t.cite(&page.source, row),
            }),
        }
    }
    if out.len() != 7 {
        return Err(ParseError::Inconsistent {
            what,
            detail: format!("{} education levels", out.len()),
        });
    }
    out.sort_by_key(|r| r.key);
    Ok(out)
}

fn cec(p: &ProgramPages) -> Result<Cec, ParseError> {
    let page = p.cec;
    let what = "CEC minimum language";
    let t = p
        .language
        .table(what, |t| t.h(0) == "FEER de la CNP" && t.h(1) == "Anglais")?;
    t.expect_cols(what, 3)?;
    let mut by_teer = Vec::new();
    for (row, cells) in t.body() {
        let (en, fr) = (clb_cell(&cells[1]), clb_cell(&cells[2]));
        match (en, fr) {
            (Some(e), Some(f)) if e == f => by_teer.push((cells[0].clone(), e, row)),
            _ => {
                return Err(ParseError::Inconsistent {
                    what,
                    detail: cells.join(" | "),
                });
            }
        }
    }
    let find = |label: &str| {
        by_teer
            .iter()
            .find(|(l, _, _)| l == label)
            .map(|(_, v, row)| Cap {
                value: u16::from(*v),
                cite: t.cite(&p.language.source, *row),
            })
            .ok_or(ParseError::MissingRow {
                what,
                label: "FEER row",
            })
    };
    Ok(Cec {
        experience: ExperienceRule {
            min_hours: text_num(
                page,
                "CEC minimum hours",
                r"au moins une année de travail ou (\d) (\d{3}) heures au total",
            )?,
            window_years: text_num(
                page,
                "CEC experience window",
                r"heures au total \(\d+ heures par semaine\) au cours des (\d+) années précédant votre demande",
            )?,
            max_hours_per_week: text_num(
                page,
                "CEC weekly cap",
                r"heures au total \((\d+) heures par semaine\) au cours des",
            )?,
        },
        // The page writes "FEER 0 or 1" (sic, English "or").
        min_clb_teer_0_1: find("FEER 0 or 1")?,
        min_clb_teer_2_3: find("FEER 2 or 3")?,
    })
}

fn fst(p: &ProgramPages) -> Result<Fst, ParseError> {
    let page = p.fst;
    let what = "FST trades";
    let c = re(r"Groupes majeurs ((?:\d{2}(?: \(à l'exception du sous-groupe majeur \d{3}\))?(?: ?,| ?, ou| ou)? ?)+)Sous-groupe (\d{4}) Groupe de base (\d{5})")
        .captures(&page.text)
        .ok_or(ParseError::TextNotFound(what))?;
    let mut trades: Vec<NocGroup> =
        re(r"(\d{2})(?: \(à l'exception du sous-groupe majeur (\d{3})\))?")
            .captures_iter(&c[1])
            .map(|g| NocGroup {
                prefix: g[1].to_owned(),
                except: g
                    .get(2)
                    .map(|m| vec![m.as_str().to_owned()])
                    .unwrap_or_default(),
            })
            .collect();
    trades.push(NocGroup {
        prefix: c[2].to_owned(),
        except: vec![],
    });
    trades.push(NocGroup {
        prefix: c[3].to_owned(),
        except: vec![],
    });
    for g in &trades {
        if !g.except.iter().all(|e| e.starts_with(&g.prefix)) {
            return Err(ParseError::Inconsistent {
                what,
                detail: format!("{g:?}"),
            });
        }
    }

    let what = "FST minimum language";
    let t = p.language.table(what, |t| {
        t.h(0) == "Langue" && t.h(1) == "Compétences linguistiques" && t.h(2) == "Niveau minimum"
    })?;
    t.expect_cols(what, 3)?;
    let mut sl = Vec::new();
    let mut rw = Vec::new();
    for (row, cells) in t.body() {
        let lvl = clb_cell(&cells[2]).ok_or_else(|| ParseError::BadNumber {
            what,
            table: t.index,
            row,
            text: cells[2].clone(),
        })?;
        match cells[1].as_str() {
            "Expression orale et compréhension de l'oral" => sl.push((lvl, row)),
            "Compréhension de l'écrit et expression écrite" => rw.push((lvl, row)),
            _ => {
                return Err(ParseError::UnexpectedRow {
                    what,
                    table: t.index,
                    row,
                    text: cells[1].clone(),
                });
            }
        }
    }
    let one = |v: &[(u8, u16)]| match v {
        [(a, row), (b, _)] if a == b => Ok(Cap {
            value: u16::from(*a),
            cite: t.cite(&p.language.source, *row),
        }),
        _ => Err(ParseError::Inconsistent {
            what,
            detail: format!("{v:?}"),
        }),
    };

    Ok(Fst {
        experience: ExperienceRule {
            min_hours: text_num(
                page,
                "FST minimum hours",
                r"au moins \d+ ans de travail à temps plein \(ou (\d) (\d{3}) heures au total\)",
            )?,
            window_years: text_num(
                page,
                "FST experience window",
                r"dans un métier spécialisé au cours des (\d+) années précédant votre demande",
            )?,
            max_hours_per_week: text_num(
                page,
                "FST weekly cap",
                r"au-delà des (\d+) heures par semaine",
            )?,
        },
        trades,
        trades_cite: page.text_cite(&c[0]),
        min_clb_speaking_listening: one(&sl)?,
        min_clb_reading_writing: one(&rw)?,
    })
}

fn funds(page: &Page) -> Result<Funds, ParseError> {
    let what = "proof of funds";
    let t = page.table(what, |t| t.h(0) == "Nombre de membres de la famille")?;
    t.expect_cols(what, 2)?;
    let updated = re(r"Mise à jour : le (.+)$")
        .captures(t.h(1))
        .map(|c| c[1].to_owned())
        .ok_or_else(|| ParseError::BadLabel {
            what,
            table: t.index,
            row: 0,
            text: t.h(1).to_owned(),
        })?;
    let amount = |row: u16, s: &str| -> Result<u32, ParseError> {
        let digits: String = s
            .trim_end_matches('$')
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .then(|| digits.parse().ok())
            .flatten()
            .ok_or_else(|| ParseError::BadNumber {
                what,
                table: t.index,
                row,
                text: s.to_owned(),
            })
    };
    let mut rows = Vec::new();
    let mut extra = None;
    for (row, cells) in t.body() {
        if let Ok(n) = cells[0].parse::<u8>() {
            rows.push(FundsRow {
                family_size: n,
                amount_cad: amount(row, &cells[1])?,
                cite: t.cite(&page.source, row),
            });
        } else if let Some(v) = caps(
            r"^Si plus de (\d+) personnes, pour chaque membre de la famille supplémentaire, ajoutez$",
            &cells[0],
        ) {
            extra = Some((v[0], amount(row, &cells[1])?, t.cite(&page.source, row)));
        } else {
            return Err(ParseError::UnexpectedRow {
                what,
                table: t.index,
                row,
                text: cells[0].clone(),
            });
        }
    }
    let (after, per_extra_member_cad, extra_cite) = extra.ok_or(ParseError::MissingRow {
        what,
        label: "Si plus de N personnes",
    })?;
    let consecutive = rows
        .iter()
        .enumerate()
        .all(|(i, r)| usize::from(r.family_size) == i + 1);
    let increasing = rows.windows(2).all(|w| w[1].amount_cad > w[0].amount_cad);
    if !consecutive || !increasing || rows.last().map(|r| r.family_size) != Some(after) {
        return Err(ParseError::Inconsistent {
            what,
            detail: "family sizes / amounts".into(),
        });
    }
    Ok(Funds {
        rows,
        per_extra_member_cad,
        extra_cite,
        updated,
    })
}

pub fn parse_programs(p: &ProgramPages) -> Result<Programs, ParseError> {
    Ok(Programs {
        fsw: fsw(p)?,
        cec: cec(p)?,
        fst: fst(p)?,
        funds: funds(p.funds)?,
    })
}
