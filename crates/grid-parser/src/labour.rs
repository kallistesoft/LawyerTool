//! Labour market integration factor (`modifications.html`) and the high-wage
//! occupation list (`salaire-eleve.html`), both in force from 2026-11-03.

use std::collections::HashSet;

use rules_engine::grid::{
    HighWageList, IsoDate, LabourMarket, Occupation, Points, Row, SingleRow, WageTier,
};

use crate::ParseError;
use crate::crs::{cell_num, labelled, ranged, re};
use crate::page::Page;

const EXPERIENCE: &[(&str, Option<WageTier>)] = &[
    (
        r"^Au moins (\d+) mois dans un ou plusieurs professions à salaire élevé dont le salaire médian est au moins le double du salaire médian moyen$",
        Some(WageTier::AtLeast2),
    ),
    (
        r"^Si vous n'êtes pas admissible aux \d+ points, au moins (\d+) mois cumulés dans un ou plusieurs professions à salaire élevé dont le salaire médian est au moins 1,5 fois le salaire médian moyen$",
        Some(WageTier::AtLeast1_5),
    ),
    (
        r"^Si vous n'êtes pas admissible aux \d+ points, au moins (\d+) mois cumulés dans un ou plusieurs professions à salaire élevé dont le salaire médian est au moins 1,3 fois le salaire médian moyen$",
        Some(WageTier::AtLeast1_3),
    ),
    (
        r"^Moins de (\d+) mois d'expérience admissible dans une profession à salaire élevé$",
        None,
    ),
];

const OFFER: &[(&str, Option<WageTier>)] = &[
    (
        r"^Un emploi dans une profession à salaire élevé offrant au moins le double du salaire médian moyen$",
        Some(WageTier::AtLeast2),
    ),
    (
        r"^Un emploi dans une profession à salaire élevé dont la rémunération correspond à au moins 1,5 fois, mais à moins de deux fois, le salaire médian moyen$",
        Some(WageTier::AtLeast1_5),
    ),
    (
        r"^Un emploi dans une profession à salaire élevé dont la rémunération correspond à au moins 1,3 fois, mais à moins de 1,5 fois, le salaire médian moyen$",
        Some(WageTier::AtLeast1_3),
    ),
    (
        r"^Un emploi qui ne relève pas d'une profession à salaire élevé$",
        None,
    ),
];

const LIST: &[(&str, WageTier)] = &[
    (
        r"^Les CNPs dont le salaire médian est au moins deux fois supérieur au salaire médian moyen$",
        WageTier::AtLeast2,
    ),
    (
        r"^Les CNPs dont le salaire médian est au moins 1,5 fois mais moins de deux fois le salaire médian moyen$",
        WageTier::AtLeast1_5,
    ),
    (
        r"^Les CNPs dont le salaire médian est au moins 1,3 fois mais moins de 1,5 fois le salaire médian moyen$",
        WageTier::AtLeast1_3,
    ),
];

/// Tiered rows; the "none" row must be worth 0 and is dropped.
fn tiered(
    page: &Page,
    t: &crate::page::Table,
    what: &'static str,
    labels: &[(&'static str, Option<WageTier>)],
) -> Result<(Vec<SingleRow<WageTier>>, Vec<String>), ParseError> {
    t.expect_cols(what, 2)?;
    let mut rows = Vec::new();
    let mut months = Vec::new();
    for (row, tier, caps) in labelled(t, what, labels)? {
        let points = cell_num(t, what, row, 1)?;
        months.extend(caps);
        match tier {
            Some(key) => rows.push(SingleRow {
                key,
                points,
                cite: t.cite(&page.source, row),
            }),
            None if points == 0 => {}
            None => {
                return Err(ParseError::BadNumber {
                    what,
                    table: t.index,
                    row,
                    text: points.to_string(),
                });
            }
        }
    }
    Ok((rows, months))
}

pub fn parse_labour_market(page: &Page) -> Result<LabourMarket, ParseError> {
    let what = "labour market: duration";
    let t = page.table(what, |t| {
        t.h(0) == "Durée de l'expérience de travail au Canada"
    })?;
    t.expect_cols(what, 3)?;
    let duration = ranged(t, what, crate::crs::parse_years)?
        .into_iter()
        .map(|(row, key)| {
            Ok(Row {
                key,
                points: Points {
                    with_spouse: cell_num(t, what, row, 1)?,
                    without_spouse: cell_num(t, what, row, 2)?,
                },
                cite: t.cite(&page.source, row),
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;

    let what = "labour market: high-wage experience";
    let t = page.table(what, |t| {
        t.h(0) == "Expérience de travail canadienne admissible"
    })?;
    let (high_wage_experience, months) = tiered(page, t, what, EXPERIENCE)?;
    let high_wage_min_months = match months
        .iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()[..]
    {
        [m] => m.parse().map_err(|_| ParseError::Inconsistent {
            what,
            detail: m.clone(),
        })?,
        _ => {
            return Err(ParseError::Inconsistent {
                what,
                detail: format!("month thresholds {months:?}"),
            });
        }
    };

    let what = "labour market: job offer";
    let t = page.table(what, |t| t.h(0) == "Catégorie d'offre d'emploi")?;
    let (job_offer, _) = tiered(page, t, what, OFFER)?;

    // Summary table: totals must equal the detail tables.
    let what = "labour market: summary";
    let t = page.table(what, |t| t.caption == "Calcul du score")?;
    t.expect_cols(what, 3)?;
    let total = re(
        r"^(\d+) points : (\d+) pour la durée de l'emploi (\d+) pour une expérience dans une profession à salaire élevé$",
    );
    let offer = re(r"^(\d+) points$");
    let max_of = |v: &[SingleRow<WageTier>]| v.iter().map(|r| r.points).max().unwrap_or(0);
    let mut max = None;
    for (row, cells) in t.body() {
        let bad = || ParseError::UnexpectedRow {
            what,
            table: t.index,
            row,
            text: cells.join(" | "),
        };
        match cells[0].as_str() {
            "Expérience de travail au Canada" => {
                let mut p = [0u16; 2];
                for (i, col) in [1, 2].into_iter().enumerate() {
                    let c = total.captures(&cells[col]).ok_or_else(bad)?;
                    let n = |k: usize| c[k].parse::<u16>().map_err(|_| bad());
                    let dur_max = duration
                        .iter()
                        .map(|r| r.points.get(i == 0))
                        .max()
                        .unwrap_or(0);
                    if n(2)? != dur_max
                        || n(3)? != max_of(&high_wage_experience)
                        || n(1)? != n(2)? + n(3)?
                    {
                        return Err(ParseError::Inconsistent {
                            what,
                            detail: cells[col].clone(),
                        });
                    }
                    p[i] = n(1)?;
                }
                max = Some(Row {
                    key: (),
                    points: Points {
                        with_spouse: p[0],
                        without_spouse: p[1],
                    },
                    cite: t.cite(&page.source, row),
                });
            }
            "Offre d'emploi admissible" => {
                for col in [1, 2] {
                    let c = offer.captures(&cells[col]).ok_or_else(bad)?;
                    if c[1].parse::<u16>().ok() != Some(max_of(&job_offer)) {
                        return Err(ParseError::Inconsistent {
                            what,
                            detail: cells[col].clone(),
                        });
                    }
                }
            }
            _ => return Err(bad()),
        }
    }
    let max = max.ok_or(ParseError::MissingRow {
        what,
        label: "Expérience de travail au Canada",
    })?;

    Ok(LabourMarket {
        max,
        duration,
        high_wage_experience,
        high_wage_min_months,
        job_offer,
    })
}

fn french_date(s: &str) -> Option<IsoDate> {
    let c = re(r"Entrée en vigueur le (\d{1,2})(?:er)? (\w+) (\d{4})").captures(s)?;
    let months = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    let m = months.iter().position(|m| *m == &c[2])? + 1;
    let d: u8 = c[1].parse().ok()?;
    IsoDate::parse(&format!("{}-{m:02}-{d:02}", &c[3]))
}

pub fn parse_high_wage(page: &Page) -> Result<HighWageList, ParseError> {
    let what = "high-wage occupations";
    let effective_from =
        french_date(&page.text).ok_or(ParseError::TextNotFound("high-wage list effective date"))?;
    let noc = re(r"^\d{5}$");
    let mut seen = HashSet::new();
    let mut occupations = Vec::new();
    for (pattern, tier) in LIST {
        let r = re(pattern);
        let t = page.table(what, |t| r.is_match(&t.caption))?;
        t.expect_cols(what, 2)?;
        if !t.h(0).starts_with("Code") {
            return Err(ParseError::BadLabel {
                what,
                table: t.index,
                row: 0,
                text: t.h(0).to_owned(),
            });
        }
        for (row, cells) in t.body() {
            let code = &cells[0];
            if !noc.is_match(code) || !seen.insert(code.clone()) {
                return Err(ParseError::BadLabel {
                    what,
                    table: t.index,
                    row,
                    text: code.clone(),
                });
            }
            occupations.push(Occupation {
                noc: code.clone(),
                tier: *tier,
                cite: t.cite(&page.source, row),
            });
        }
    }
    // Every table on the page must be one of the three tiers.
    if page.tables.len() != LIST.len() {
        return Err(ParseError::Inconsistent {
            what,
            detail: format!("{} tables on page", page.tables.len()),
        });
    }
    Ok(HighWageList {
        effective_from,
        occupations,
    })
}
