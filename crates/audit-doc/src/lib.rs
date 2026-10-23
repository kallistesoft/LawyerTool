//! The lawyer's « AUDIT ÉCLAIR D'ÉLIGIBILITÉ », generated from one intake-form
//! response, as a Word document she finalizes before sending.
//!
//! The structure, fixed texts and rules come from her own audits (version
//! of 2026-10-22), made consistent (rulings of 2026-10-22): age over 35
//! warns, the funds threshold follows the household size, the French-test
//! status follows the answer, the spouse's diploma row follows the spouse's
//! answer and a relative in Canada is always mentioned. The occupation code
//! (CNP → FEER) is set by the lawyer in the app; without it, the document
//! keeps highlighted blanks for her to fill. No personal score is printed.

mod facts;

use applicant::IsoDate;
use report::rich::{Cell, Doc, Item, Para, Run};
use rules_engine::programs::Funds as FundsTable;

pub use facts::{Facts, FormRecord, Lawyer, Level};
use facts::{FrenchTest, Funds};

/// Values that change with IRCC news, not with the client.
#[derive(Debug, Clone)]
pub struct Context {
    pub today: IsoDate,
    /// Latest French-language draw cutoff (CRS), for the Entrée express line.
    pub french_cutoff: Option<u16>,
    /// IRCC settlement-funds table (from the approved grid).
    pub funds: Option<FundsTable>,
}

/// Cutoff quoted when no French draw is known (her audits of 22 Oct).
pub const DEFAULT_FRENCH_CUTOFF: u16 = 387;

// Colours of her template.
const NAVY: &str = "0B3142";
const GOLD: &str = "FFD24A";
const INK: &str = "1F2937";
const TITLE_BLUE: &str = "1B4F6B";
const ROW_ALT: &str = "F2F5F9";
const GREEN: &str = "2E9E2E";
const GREEN_TEXT: &str = "2E7D32";
const OLIVE: &str = "8C8C2A";
const YELLOW: &str = "D9A400";
const ORANGE: &str = "F07800";
const RED: &str = "C62828";
const FEAS_HEAD: &str = "0B6623";
const FEAS_QC: &str = "FDE8E8";
const FEAS_OUT: &str = "E8F5E9";
const SPOUSE_TITLE: &str = "4A90D9";
const SPOUSE_FILL: &str = "DCE9F7";
const ROAD: &str = "C0501A";
const ROAD_ALT: &str = "FBE3D6";
const BILAN: &str = "7A7A00";

/// Indicator dot and the colour of its impact text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Green,
    Olive,
    Yellow,
    Orange,
}

impl Tone {
    fn dot(self) -> &'static str {
        match self {
            Tone::Green => GREEN,
            Tone::Olive => OLIVE,
            Tone::Yellow => YELLOW,
            Tone::Orange => ORANGE,
        }
    }
    fn text(self) -> &'static str {
        match self {
            Tone::Green => GREEN_TEXT,
            Tone::Orange => RED,
            other => other.dot(),
        }
    }
}

fn r(t: impl Into<String>) -> Run {
    Run::new(t)
}

fn b(t: impl Into<String>) -> Run {
    Run::new(t).bold()
}

/// A blank or a value the lawyer must check (yellow highlight).
fn todo(t: impl Into<String>) -> Run {
    Run::new(t).bold().highlight()
}

fn p(runs: Vec<Run>) -> Para {
    Para::new(runs)
}

fn bullet(runs: Vec<Run>) -> Para {
    Para::new(runs).bullet(1)
}

fn months_fr(m: u8) -> &'static str {
    [
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
    ][usize::from(m.clamp(1, 12) - 1)]
}

/// « 22 octobre 2026 ».
pub fn long_date(d: &IsoDate) -> String {
    let s = d.as_str();
    let (y, m, day) = (
        &s[0..4],
        s[5..7].parse::<u8>().unwrap_or(1),
        s[8..10].trim_start_matches('0'),
    );
    format!("{day} {} {y}", months_fr(m))
}

/// `Audit_<NOM>.docx`. The form asks « nom et prénom », so the last name is
/// the words written in capitals (« LE BAYEC Argentina »), else the first word.
pub fn file_name(full_name: &str) -> String {
    let words: Vec<&str> = full_name.split_whitespace().collect();
    let is_caps = |w: &&str| {
        w.chars().filter(|c| c.is_alphabetic()).count() > 1 && w.chars().all(|c| !c.is_lowercase())
    };
    let caps: Vec<&str> = words.iter().copied().filter(is_caps).collect();
    let pick = if caps.is_empty() {
        words.first().copied().unwrap_or("").to_owned()
    } else {
        caps.join("-")
    };
    let clean: String = pick
        .chars()
        .filter_map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' => Some(c.to_ascii_uppercase()),
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' => Some('E'),
            'à' | 'â' | 'À' => Some('A'),
            'î' | 'ï' => Some('I'),
            'ô' => Some('O'),
            'ù' | 'û' | 'ü' => Some('U'),
            'ç' => Some('C'),
            _ => None,
        })
        .collect();
    format!(
        "Audit_{}.docx",
        if clean.is_empty() {
            "CLIENT".into()
        } else {
            clean
        }
    )
}

fn funds_needed(ctx: &Context, size: u8) -> Option<u32> {
    ctx.funds.as_ref()?.required(size).map(|(x, _)| x)
}

fn money(x: u32) -> String {
    let s = x.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    out
}

/// Builds the audit for one respondent.
pub fn build(full_name: &str, form: &FormRecord, ctx: &Context) -> Doc {
    let f = Facts::of(form);
    let mut items = Vec::new();
    header(&mut items, full_name, ctx);
    synthesis(&mut items, &f, ctx);
    items.push(Item::PageBreak);
    items.push(Item::Boxed(vec![p(vec![
        b("Remarque préalable").color(TITLE_BLUE),
        r(" : L’analyse contenue dans cet audit repose sur les "),
        b("informations communiquées par le client à la date du questionnaire."),
        r(" Les politiques d'immigration provinciales et fédérales étant sujettes à des modifications fréquentes et sans préavis par arrêté ministériel ou instruction générale, les stratégies préconisées dans ce document constituent une évaluation de faisabilité à un instant T et ne sauraient garantir la délivrance d'un visa, d'un permis ou d'un statut de résident permanent, décision qui relève du pouvoir souverain exclusif d'IRCC et du MIFI."),
    ])
    .justify()]));
    feasibility(&mut items, &f, ctx);
    if f.married {
        items.push(Item::PageBreak);
        spouse(&mut items, &f);
    }
    items.push(Item::PageBreak);
    roadmap(&mut items, &f);
    bilan(&mut items, &f);
    closing(&mut items);
    Doc { items }
}

fn header(items: &mut Vec<Item>, name: &str, ctx: &Context) {
    items.push(Item::Banner {
        lines: vec![
            b("CABINET D’AVOCAT").color(GOLD).size(40),
            b("POUGHON IMMIGRATION").color(GOLD).size(40),
        ],
        fill: NAVY,
    });
    items.push(Item::Para(
        p(vec![r(
            "Membre du Barreau du Québec – Numéro de membre 381467-0",
        )])
        .center()
        .after(200),
    ));
    items.push(Item::Para(
        p(vec![
            b("AUDIT ÉCLAIR D’ÉLIGIBILITÉ").color(TITLE_BLUE).size(40),
        ])
        .center(),
    ));
    items.push(Item::Para(
        p(vec![b(name.trim()).color(TITLE_BLUE).size(36)]).center(),
    ));
    items.push(Item::Para(
        p(vec![
            b(format!("Date de l’évaluation – {}", long_date(&ctx.today)))
                .color(TITLE_BLUE)
                .size(24),
        ])
        .center()
        .after(300),
    ));
}

fn row(i: usize, label: &str, eval: Vec<Run>, tone: Tone, impact: Vec<Run>) -> Vec<Cell> {
    let fill = if i % 2 == 1 { Some(ROW_ALT) } else { None };
    let mut cells = vec![
        Cell::runs(vec![b(label).color(TITLE_BLUE)]),
        Cell::runs(eval),
        Cell::new(vec![p(vec![r("●").color(tone.dot()).size(32)]).center()]),
        Cell::runs(
            impact
                .into_iter()
                .map(|x| {
                    if x.color.is_none() {
                        x.color(tone.text())
                    } else {
                        x
                    }
                })
                .collect(),
        ),
    ];
    if let Some(fl) = fill {
        cells = cells.into_iter().map(|c| c.fill(fl)).collect();
    }
    cells
}

fn head_cells(titles: &[&str], fill: &'static str, color: &'static str) -> Vec<Cell> {
    titles
        .iter()
        .map(|t| Cell::new(vec![p(vec![b(*t).color(color).size(17)]).center()]).fill(fill))
        .collect()
}

fn synthesis(items: &mut Vec<Item>, f: &Facts, ctx: &Context) {
    items.push(Item::Heading {
        text: "Synthèse du profil du candidat".into(),
        color: INK,
    });
    let mut rows = Vec::new();

    // Âge & foyer.
    let foyer = if f.married {
        "Marié ou union de fait"
    } else {
        "Célibataire"
    };
    let age = f
        .age
        .map_or_else(|| todo("[âge à vérifier]"), |a| r(format!("{a} ans")));
    let (tone, impact) = match f.age {
        Some(a) if a > 35 => (
            Tone::Olive,
            "Après 35 ans, on perd des points pour l'Entrée express.",
        ),
        _ => (Tone::Green, "Atout majeur"),
    };
    rows.push(row(
        rows.len(),
        "ÂGE & FOYER",
        vec![age, r(format!(" — {foyer}"))],
        tone,
        vec![r(impact)],
    ));

    // Nationalité.
    let nat = f.a("nationality");
    let res = f.a("residence");
    let eval = vec![r(format!(
        "Citoyenneté {} (Résidence : {})",
        if nat.is_empty() { "à préciser" } else { nat },
        if res.is_empty() { "à préciser" } else { res }
    ))];
    let (tone, impact) = if f.francophone() {
        (Tone::Green, "Atout Majeur (Mobilité Francophone C16)")
    } else {
        (
            Tone::Olive,
            "Mobilité francophone : NCLC 5 en français requis",
        )
    };
    rows.push(row(rows.len(), "NATIONALITÉ", eval, tone, vec![r(impact)]));

    // Diplômes.
    let eca = if f.eca_started {
        "ÉDE faite ou en cours"
    } else {
        "ÉDE pas encore faite"
    };
    let diploma = f.a("education");
    let eval = vec![
        if diploma.is_empty() {
            todo("[diplôme à préciser]")
        } else {
            r(diploma)
        },
        r(format!(" ({eca})")),
    ];
    let (tone, impact) = if f.diploma_in_progress {
        (Tone::Olive, "À confirmer")
    } else {
        (Tone::Green, "Exigence minimale acquise")
    };
    rows.push(row(rows.len(), "DIPLÔMES", eval, tone, vec![r(impact)]));

    // Profession.
    let job = f.a("job");
    let seniority = f.seniority();
    let mut eval = vec![r(if job.is_empty() {
        "Métier à préciser".to_owned()
    } else {
        job.to_owned()
    })];
    if !seniority.is_empty() {
        eval.push(r(format!(", {seniority}")));
    }
    eval.push(r(" (CNP "));
    eval.push(match &f.noc {
        Some(n) if n.validated => r(n.code.clone()),
        Some(n) => todo(format!("{} à valider", n.code)),
        None => todo("à valider"),
    });
    eval.push(r(")"));
    let (tone, impact) = match &f.noc {
        Some(n) if n.feer == 0 => (Tone::Green, vec![r("Métier très qualifié (FEER 0)")]),
        Some(n) if n.qualified() => (
            Tone::Green,
            vec![r(format!("Métier qualifié (FEER {})", n.feer))],
        ),
        Some(n) => (
            Tone::Yellow,
            vec![r(format!(
                "Métier peu qualifié (FEER {}) et non admissible à l'Entrée express",
                n.feer
            ))],
        ),
        None => (
            Tone::Green,
            vec![
                r("Métier qualifié ? "),
                todo("CNP À DÉFINIR IMPÉRATIVEMENT"),
            ],
        ),
    };
    rows.push(row(rows.len(), "PROFESSION", eval, tone, impact));

    // Langues.
    let test = match f.french_test {
        FrenchTest::Done => "Test réalisé",
        FrenchTest::InProgress => "Test en cours",
        FrenchTest::ToTake => "Test officiel à passer",
    };
    let fr = f.french.map_or("niveau à préciser", Level::fr);
    let en = f.english.map_or("niveau à préciser", Level::fr);
    let eval = vec![r(format!(
        "Français ({} – {test}) / Anglais ({en})",
        capitalize(fr)
    ))];
    let (tone, impact) = if f.francophone() {
        (Tone::Green, "Atout stratégique majeur")
    } else {
        (Tone::Orange, "Français à renforcer (NCLC 7 visé)")
    };
    rows.push(row(rows.len(), "LANGUES", eval, tone, vec![r(impact)]));

    // Fonds.
    let household = 1 + u8::from(f.married);
    let (eval, tone, impact) = match f.funds {
        Funds::Enough(n, x) => (
            "Conforme".to_owned(),
            Tone::Green,
            format!("Au moins {} $ CAD d'économies pour {n}", money(x)),
        ),
        Funds::NotEnough => (
            "Non conforme".to_owned(),
            Tone::Orange,
            format!(
                "Au moins {} $ CAD d'économies requis pour {household} personne{}. Attention : Les fonds doivent être disponibles et non grevés de dettes. Attestation bancaire de 6 mois à justifier.",
                funds_needed(ctx, household).map_or_else(|| "[montant]".into(), money),
                if household > 1 { "s" } else { "" }
            ),
        ),
        Funds::Unknown => (
            "Non renseigné".to_owned(),
            Tone::Olive,
            format!(
                "Minimum {} $ CAD requis pour {household} personne{} pour la RP",
                funds_needed(ctx, household).map_or_else(|| "[montant]".into(), money),
                if household > 1 { "s" } else { "" }
            ),
        ),
    };
    rows.push(row(
        rows.len(),
        "FONDS DISPONIBLES",
        vec![r(eval)],
        tone,
        vec![r(impact)],
    ));

    // Province.
    let prov = f.a("province");
    let (tone, impact) = if f.quebec_only {
        (Tone::Orange, "Immigration complexe et longue")
    } else {
        (Tone::Green, "Atout stratégique majeur")
    };
    rows.push(row(
        rows.len(),
        "PROVINCE VISÉE",
        vec![r(if prov.is_empty() {
            "À préciser"
        } else {
            prov
        })],
        tone,
        vec![r(impact)],
    ));

    items.push(Item::Table {
        widths: vec![22, 34, 13, 31],
        header: Some(head_cells(
            &[
                "CRITÈRE PROFIL",
                "ÉVALUATION RÉELLE DU PROFIL",
                "INDICATEUR",
                "ANALYSE DE L'IMPACT",
            ],
            NAVY,
            GOLD,
        )),
        rows,
        rule: INK,
    });
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}

fn english_bullet(f: &Facts) -> Vec<Run> {
    match f.english {
        Some(Level::Bilingual | Level::Native) => {
            vec![
                b("🗣️ Votre anglais bilingue est un très gros atout"),
                r(" dans tout le Canada."),
            ]
        }
        Some(Level::Intermediate) => {
            let mut v = vec![b("🗣️ Votre anglais intermédiaire est un atout.")];
            if f.alberta {
                v.push(r(
                    " À consolider pour trouver une offre d’emploi en Alberta.",
                ));
            }
            v
        }
        _ => {
            let where_ = if f.alberta {
                "pour l'Alberta"
            } else if f.nb {
                "même pour le Nouveau-Brunswick"
            } else {
                "hors Québec"
            };
            vec![
                b("🗣️ Votre anglais débutant doit être amélioré"),
                r(format!(" {where_}.")),
            ]
        }
    }
}

fn feasibility(items: &mut Vec<Item>, f: &Facts, ctx: &Context) {
    items.push(Item::Heading {
        text: "Faisabilité des voies d’immigration".into(),
        color: FEAS_HEAD,
    });
    let mut rows = Vec::new();
    if f.quebec {
        rows.push(vec![
            Cell::runs(vec![b("📍 AU QUÉBEC")]).fill(FEAS_QC),
            Cell::new(vec![
                p(vec![r("●").color(ORANGE).size(28), b(" COMPLEXE").color(ORANGE)]),
                p(vec![r("(20%)")]),
                p(vec![r("Voie complexe sans expérience locale et sans offre d’emploi")]),
            ])
            .fill(FEAS_QC),
            Cell::new(vec![
                p(vec![b("❌ PSTQ / PEQ : Resserrement MIFI. Priorité exclusive aux diplômés/travailleurs sur place.").color("E65100")]),
                p(vec![b("❌ Permis temporaire : Délais importants, EIMT/CAQ lourde pour l'employeur.").color("E65100")]),
            ])
            .fill(FEAS_QC),
        ]);
    }
    let cutoff = ctx.french_cutoff.unwrap_or(DEFAULT_FRENCH_CUTOFF);
    let (level, pct, label) = if f.low_skilled() {
        ("MODÉRÉ", "(50%)", "Via la mobilité francophone")
    } else {
        ("ÉLEVÉ", "(80%)", "Voie Royale Recommandée")
    };
    let mut analysis = vec![
        p(vec![
            b("🚀 1. Mobilité Francophone (Permis C16 - Prioritaire)").color(GREEN_TEXT),
            r(" : Aucune limite d'âge, dispense d'EIMT pour l'employeur.").color(GREEN_TEXT),
        ]),
        p(if f.low_skilled() {
            vec![
                b("🎯 2. Entrée Express").color(GREEN_TEXT),
                r(format!(
                    " : après de l’expérience dans un métier qualifié (FEER 0 à 3) ; tirages cibles francophones (~{cutoff} pts)."
                ))
                .color(GREEN_TEXT),
            ]
        } else {
            vec![
                b("🎯 2. Entrée Express").color(GREEN_TEXT),
                r(format!(
                    " : Tirages cibles Francophones très favorables (~{cutoff} pts)."
                ))
                .color(GREEN_TEXT),
            ]
        }),
        p(english_bullet(f)
            .into_iter()
            .map(|x| x.color(GREEN_TEXT))
            .collect()),
    ];
    if f.english.is_none_or(|l| l < Level::Bilingual) || f.nb {
        analysis.push(p(vec![
            b("📌 3. Province officiellement Bilingue : le Nouveau Brunswick (NB)").color(GREEN_TEXT),
            r(" : Transition douce pour la langue et accès rapide à la RP après 1 an. D’autres provinces aident les francophones à s’implanter avec la résidence permanente à la clef. Le programme PICA (incluant le NB) est également très pertinent à creuser, avec beaucoup d’employeurs participants.").color(GREEN_TEXT),
        ]));
    }
    rows.push(vec![
        Cell::runs(vec![b("📍 HORS QUÉBEC\n(FÉDÉRAL & PROVINCES)")]).fill(FEAS_OUT),
        Cell::new(vec![
            p(vec![
                r("●").color(GREEN).size(28),
                b(format!(" {level}")).color(GREEN_TEXT),
            ]),
            p(vec![b(pct).color(GREEN_TEXT)]),
            p(vec![r(label)]),
        ])
        .fill(FEAS_OUT),
        Cell::new(analysis).fill(FEAS_OUT),
    ]);
    items.push(Item::Table {
        widths: vec![24, 24, 52],
        header: Some(head_cells(
            &[
                "OPTION / DESTINATION",
                "NIVEAU DE FAISABILITÉ",
                "ANALYSE STRATÉGIQUE & POINTS CLÉS",
            ],
            FEAS_HEAD,
            "FFFFFF",
        )),
        rows,
        rule: INK,
    });
}

fn bar(pct: u8) -> String {
    let filled = usize::from(pct) * 14 / 100;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(14 - filled))
}

fn progress(label: &str, pct: u8) -> Cell {
    let dot = match pct {
        100 => "●",
        0 => "○",
        _ => "◔",
    };
    Cell::new(vec![
        p(vec![r(label)]).center(),
        p(vec![r(dot).color(INK)]).center(),
        p(vec![r(bar(pct)).color(INK).size(16)]).center(),
        p(vec![b(format!("{pct}%"))]).center(),
    ])
}

fn impact(word: &str, tone: Tone) -> Cell {
    Cell::new(vec![
        p(vec![r("●").color(tone.dot()).size(28)]).center(),
        p(vec![b(word).color(tone.text())]).center(),
    ])
}

fn spouse(items: &mut Vec<Item>, f: &Facts) {
    items.push(Item::Heading {
        text: "Audit sur-mesure & impact stratégique du conjoint".into(),
        color: SPOUSE_TITLE,
    });
    let mut intro = Vec::new();
    if let Some(a) = f.spouse_age {
        intro.push(format!("{a} ans"));
    }
    for k in ["spouse_nationality", "spouse_education"] {
        if !f.a(k).is_empty() {
            intro.push(f.a(k).to_lowercase());
        }
    }
    if let Some(l) = f.spouse_english {
        intro.push(format!("anglais {}", l.fr()));
    }
    if !f.a("spouse_job").is_empty() {
        intro.push(f.a("spouse_job").to_lowercase());
    }
    if !f.a("spouse_experience").is_empty() {
        intro.push(format!("ancienneté {}", f.a("spouse_experience")));
    }
    items.push(Item::Para(bullet(vec![
        b("CONJOINT : "),
        r(format!("{}.", intro.join(", "))),
    ])));

    let job = if f.a("job").is_empty() {
        "Votre métier"
    } else {
        f.a("job")
    };
    let mut rows = Vec::new();
    // Open work permit for the spouse.
    rows.push(match &f.noc {
        Some(n) if n.qualified() => vec![
            Cell::runs(vec![b("PERMIS DE TRAVAIL OUVERT (PTO)")]),
            Cell::runs(vec![
                b(format!("{job} : ")),
                r(format!(
                    "Poste {}qualifié (FEER {}). Éligible code de dispense C16.",
                    if n.feer == 0 { "très " } else { "" },
                    n.feer
                )),
            ]),
            progress("Potentiel PTO :", 100),
            impact("ACQUIS", Tone::Green),
            Cell::runs(vec![
                r("Votre métier donne droit à un "),
                b("Permis de Travail Ouvert pour le Conjoint"),
            ]),
        ],
        Some(n) => vec![
            Cell::runs(vec![b("PERMIS DE TRAVAIL OUVERT (PTO)")]),
            Cell::runs(vec![
                b(format!("{job} : ")),
                r(format!("Poste peu qualifié (FEER {}).", n.feer)),
            ]),
            progress("Potentiel PTO :", 0),
            impact("NON ADMISSIBLE", Tone::Orange),
            Cell::runs(vec![
                r("Le conjoint n’obtient pas de permis ouvert avec un métier FEER 4 ou 5.")
                    .color(RED),
            ]),
        ],
        None => vec![
            Cell::runs(vec![b("PERMIS DE TRAVAIL OUVERT (PTO)")]),
            Cell::runs(vec![
                b(format!("{job} : ")),
                todo("FEER à définir selon la CNP"),
            ]),
            progress("Potentiel PTO :", 25),
            impact("DÉCISIF", Tone::Green),
            Cell::runs(vec![
                r("À définir précisément ; un métier FEER 0 à 3 donne droit à un "),
                b("Permis de Travail Ouvert pour le Conjoint"),
            ]),
        ],
    });
    // Spouse's French.
    rows.push(vec![
        Cell::runs(vec![b("FRANÇAIS DU CONJOINT")]),
        Cell::runs(if f.spouse_test {
            vec![r("Test de langue fait : attendre le résultat.")]
        } else {
            vec![
                r("Test de "),
                b("français"),
                r(" à planifier pour viser B2 (NCLC 7+)."),
            ]
        }),
        progress("Niveau Actuel :", 25),
        impact("DÉCISIF", Tone::Green),
        Cell::runs(vec![r("Un résultat NCLC 7+ apporte un max de points.")]),
    ]);
    // Spouse's diploma.
    let dip = if f.a("spouse_education").is_empty() {
        "À préciser"
    } else {
        f.a("spouse_education")
    };
    rows.push(match f.spouse_has_bac {
        Some(false) => vec![
            Cell::runs(vec![b("ÉQUIVALENCE DIPLÔME DU CONJOINT (ÉDE)")]),
            Cell::runs(vec![r(dip)]),
            progress("Utilité ÉDE :", 0),
            impact("PAS UTILE", Tone::Orange),
            Cell::runs(vec![
                r("N'apporte pas de point au demandeur principal.").color(RED),
            ]),
        ],
        _ => vec![
            Cell::runs(vec![b("ÉQUIVALENCE DIPLÔME DU CONJOINT (ÉDE)")]),
            Cell::runs(vec![r(dip)]),
            progress("Utilité ÉDE :", 100),
            impact("DÉCISIF", Tone::Green),
            Cell::runs(vec![r("Apporte des points au demandeur principal.")]),
        ],
    });
    // Funds.
    let (status, pct, word, tone) = match f.funds {
        Funds::Enough(..) => ("Conforme", 100, "ACQUIS", Tone::Green),
        Funds::NotEnough => ("Non conforme", 0, "À CONSOLIDER", Tone::Orange),
        Funds::Unknown => ("?", 0, "?", Tone::Orange),
    };
    rows.push(vec![
        Cell::runs(vec![b("AUTONOMIE FINANCIÈRE")]),
        Cell::runs(vec![r(status)]),
        progress(if pct == 100 { "Fonds :" } else { "Inconnu :" }, pct),
        impact(word, tone),
        Cell::new(vec![
            p(vec![r("Un seuil financier minimal est exigé pour la RP.")]),
            p(vec![b("Attention"), r(" : Les fonds doivent être disponibles et non grevés de dettes. Attestation bancaire de 6 mois à justifier.")]),
        ]),
    ]);
    let rows = rows
        .into_iter()
        .map(|r| r.into_iter().map(|c| c.fill(SPOUSE_FILL)).collect())
        .collect();
    items.push(Item::Table {
        widths: vec![17, 22, 22, 14, 25],
        header: Some(head_cells(
            &[
                "CRITÈRE PROFIL",
                "SITUATION DU CANDIDAT",
                "AVANCEMENT",
                "IMPACT",
                "ANALYSE STRATÉGIQUE POUR LE CONJOINT",
            ],
            SPOUSE_FILL,
            INK,
        )),
        rows,
        rule: NAVY,
    });
}

struct Step {
    title: Vec<Run>,
    actions: Vec<Para>,
    who: String,
    delay: String,
    result: Vec<Run>,
}

fn roadmap(items: &mut Vec<Item>, f: &Facts) {
    items.push(Item::Heading {
        text: "Feuille de route & plan d’action".into(),
        color: ROAD,
    });
    let who = if f.married {
        "Requérant + conjoint"
    } else {
        "Requérant"
    };
    let mut steps: Vec<Step> = Vec::new();

    if f.low_skilled() && f.under_36() && !f.married {
        steps.push(Step {
            title: vec![b("PRÉLIMINAIRE\n\nPVT")],
            actions: vec![bullet(vec![
                r("Vous pourriez vous inscrire au "),
                b("PVT (Programme EIC)"),
                r(". C'est un tirage au sort mais c'est un programme qui vous permettrait d'obtenir un permis de travail ouvert (PTO) dans TOUTES les provinces du Canada."),
            ])],
            who: "Requérant".into(),
            delay: "Très aléatoire, inscription du profil valable un an et renouvelable jusqu'aux 35 ans du candidat.".into(),
            result: vec![r("📄 Permis de travail ouvert de 2 ans.")],
        });
    }

    let english_action = match f.english {
        Some(Level::Bilingual | Level::Native) => vec![b("Test d’anglais"), r(" opportun")],
        Some(Level::Intermediate) => vec![b("Test d’anglais"), r(" opportun si niveau suffisant")],
        _ => vec![b("Test d’anglais"), r(" non opportun")],
    };
    if f.french_test == FrenchTest::Done {
        steps.push(Step {
            title: vec![b("TEST D’ANGLAIS\n(NCLC 5+)")],
            actions: vec![
                bullet(vec![r(
                    "Test de français réalisé : joindre l’attestation officielle.",
                )]),
                bullet(english_action),
                bullet(vec![r("Au choix : CELPIP / IELTS / PTE Core.")]),
            ],
            who: who.into(),
            delay: "1-2 mois".into(),
            result: vec![r("📄 Attestations officielles de langue.")],
        });
    } else {
        steps.push(Step {
            title: vec![b(
                "TEST DE FRANÇAIS\n(NCLC 9+) &\nTEST D’ANGLAIS\n(NCLC 5+)",
            )],
            actions: vec![
                bullet(vec![
                    r("Passer un test de français officiel "),
                    b("TEF Canada ou TCF Canada"),
                    r("."),
                ]),
                bullet(vec![
                    r("Viser un score minimal de "),
                    b("NCLC 7"),
                    r(" (idéalement NCLC 9+ / B2-C1) en français."),
                ]),
                bullet(vec![
                    b("OBLIGATOIRE"),
                    r(" pour justifier de la Mobilité Francophone (NCLC 5 requis)"),
                ]),
                bullet(english_action),
            ],
            who: who.into(),
            delay: "1-2 mois".into(),
            result: vec![r("📄 Attestation officielle NCLC 9+ en français.")],
        });
    }

    let regulated: Vec<Run> = match &f.lawyer.regulated {
        Some(t) if !t.trim().is_empty() => vec![b(t.trim().to_owned())],
        _ => vec![todo("[réglementée ou non : à vérifier selon la CNP]")],
    };
    let mut eca = vec![
        bullet(vec![
            r("Lancement de "),
            b("l'ÉDE"),
            r(" (WES/ICAS) pour valoriser le diplôme auprès des employeurs "),
            b("et si vous tentez l’Entrée express."),
        ]),
        bullet(
            [vec![r("Attention aux métiers réglementées (Près de 20% au Canada) : Votre profession ")], regulated, vec![r(".")]]
                .concat(),
        ),
    ];
    if f.married {
        eca.push(bullet(vec![
            r("Profession du conjoint : "),
            todo("[ordre professionnel à vérifier]"),
        ]));
    }
    steps.push(Step {
        title: vec![b("ÉVALUATION\nDES DIPLÔMES")],
        actions: eca,
        who: format!("{who} et avocat si besoin"),
        delay: "2-3 mois".into(),
        result: vec![r("📄 Rapport d'ÉDE officiel (Pas important pour la mobilité francophone mais indispensable pour l’Entrée express.)")],
    });

    steps.push(Step {
        title: vec![b(if f.quebec {
            "DÉMARCHAGE &\nPERMIS C16\n(mobilité francophone donc hors QUÉBEC)"
        } else {
            "DÉMARCHAGE &\nPERMIS C16"
        })],
        actions: vec![
            bullet(vec![r("Préparation d'un "), b("CV canadien.").link("https://www.guichetemplois.gc.ca/rapports-tendances/conseils-carriere/rediger-cv")]),
            bullet(vec![
                r("Candidatures "),
                b("ciblées"),
                r(" auprès des entreprises dans votre domaine (Voir en annexe pour la recherche d’emploi sur guichet Emploi)."),
            ]),
            bullet(vec![r("Mise en avant expresse de la "), b("« Dispense d'EIMT »"), r(" (Code C16).")]),
        ],
        who: "Client + avocat si besoin".into(),
        delay: "2-4 mois".into(),
        result: vec![r("📄 Offre d'emploi validée + Permis C16 délivré.")],
    });

    if f.low_skilled() {
        steps.push(Step {
            title: vec![b("CRÉATION D'XP\n& RP")],
            actions: vec![
                bullet(vec![
                    b("Cumul d'expérience professionnelle"),
                    r(" au Canada, en visant un poste qualifié (FEER 3 a minima)."),
                ]),
                bullet(vec![
                    r("Dépôt de la demande de "),
                    b("Résidence Permanente"),
                    r(" quand vous aurez de l'ancienneté dans une profession qualifiée."),
                ]),
            ],
            who: "Client + avocat si besoin".into(),
            delay: "entre 12 et 36 mois selon la procédure envisagée.".into(),
            result: vec![r("📄 Statut de Résident Permanent (RP) obtenu.")],
        });
    } else {
        steps.push(Step {
            title: vec![b("INSTALLATION &\nRP")],
            actions: vec![
                bullet(vec![r("Arrivée au Canada, "), b("démarches"), r(" d'installation (NAS, logement).")]),
                bullet(vec![r("Prise de poste.")]),
                bullet(vec![
                    r("Dépôt de la demande de "),
                    b("Résidence Permanente"),
                    r(" (Entrée Express Francophone ou programme provincial), "),
                    b("après vérification du score SCG et des conditions réunies pour l’Entrée express").italic().color(RED),
                    r(if f.quebec { " - seulement hors QC." } else { "." }),
                ]),
            ],
            who: "Client + avocat si besoin".into(),
            delay: "entre 8-30 mois selon la procédure envisagée.".into(),
            result: vec![r("📄 Statut de Résident Permanent (RP) obtenu.")],
        });
    }

    let preliminary = steps
        .first()
        .and_then(|s| s.title.first())
        .is_some_and(|t| t.text.starts_with("PRÉLIMINAIRE"));
    let rows = steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let fill = if i % 2 == 1 { ROAD_ALT } else { "FFFFFF" };
            let number = if preliminary && i == 0 {
                String::new()
            } else {
                format!("📌 ÉTAPE {}\n\n", i + 1 - usize::from(preliminary))
            };
            let mut title = vec![b(number).color(TITLE_BLUE)];
            title.extend(s.title.into_iter().map(|t| t.color(TITLE_BLUE)));
            vec![
                Cell::runs(title).fill(fill),
                Cell::new(s.actions).fill(fill),
                Cell::new(vec![
                    p(vec![r(s.who)]),
                    p(vec![r(format!("⏱️ Délais : {}", s.delay))]),
                ])
                .fill(fill),
                Cell::runs(s.result).fill(fill),
            ]
        })
        .collect();
    items.push(Item::Table {
        widths: vec![19, 41, 20, 20],
        header: Some(head_cells(
            &[
                "ÉTAPES & OBJECTIFS",
                "ACTIONS PRÉCISES À RÉALISER",
                "INTERVENANT & DÉLAIS",
                "RÉSULTAT ATTENDU",
            ],
            ROAD,
            "FFFFFF",
        )),
        rows,
        rule: NAVY,
    });
}

fn bilan(items: &mut Vec<Item>, f: &Facts) {
    items.push(Item::Heading {
        text: "Bilan de votre audit".into(),
        color: BILAN,
    });
    let concerns = f.concerns();
    if !concerns.is_empty() {
        items.push(Item::Para(p(vec![
            todo("Point à examiner avant envoi (admissibilité) : "),
            todo(concerns.join(" ; ")),
        ])));
    }
    if f.low_skilled() {
        items.push(Item::Para(p(vec![
            r("🥇 DE L'EXPÉRIENCE À LA MOBILISATION : VOTRE PLAN D'ACTION")
                .color(BILAN)
                .size(24),
        ])));
        items.push(Item::Para(
            p(vec![
                r("Soyons transparents : Votre métier "),
                b("n'est pas assez qualifié"),
                r(" pour tenter l'Entrée express (résidence permanente) dès maintenant. En revanche, il est tout à fait envisageable de "),
                b("monter en grade"),
                r(" dans votre métier et gagner en responsabilité afin d'atteindre de l'ancienneté dans un métier qualifié en "),
                b("FEER 3"),
                r(" a minima."),
            ])
            .justify(),
        ));
        items.push(Item::Para(
            p(vec![
                r("Votre priorité absolue doit être la "),
                b("recherche active"),
                r(" d'un employeur canadien et passer par une mobilité francophone (sans EIMT) dès à présent."),
            ])
            .justify(),
        ));
        if f.english.is_some_and(|l| l >= Level::Bilingual) {
            items.push(Item::Para(p(vec![r(
                "Votre niveau d'anglais est également un incroyable atout.",
            )])));
        }
        if f.relative {
            items.push(Item::Para(p(vec![r("Votre proche au Canada (citoyen ou résident permanent) pourra faciliter votre installation : précisez le lien (seul un frère ou une sœur donne des points à l’Entrée express).")])));
        }
        items.push(Item::Para(
            p(vec![r("Le chemin demandera de la méthode, de l'énergie et une prospection ciblée, mais c'est une stratégie parfaitement viable ! Il sera tout à fait possible par la suite, de solliciter une résidence permanente dès les conditions réunies !")])
                .justify(),
        ));
    } else {
        items.push(Item::Para(p(vec![
            r("🥇 FÉLICITATIONS : UN EXCELLENT POTENTIEL !")
                .color(BILAN)
                .size(24),
        ])));
        let mut atouts = Vec::new();
        if f.noc.as_ref().is_some_and(|n| n.feer <= 1) {
            atouts.push("votre profession très qualifiée");
        } else {
            atouts.push("votre profession");
        }
        if f.age.is_some_and(|a| a <= 35) {
            atouts.push("votre âge");
        }
        atouts.push("votre expérience professionnelle");
        if f.english.is_some_and(|l| l >= Level::Bilingual) {
            atouts.push("votre bilinguisme");
        }
        items.push(Item::Para(
            p(vec![
                r(format!(
                    "Votre profil rassemble des atouts majeurs ({}). Votre potentiel de réussite pour immigrer au Canada ",
                    atouts.join(", ")
                )),
                b("HORS QUÉBEC"),
                r(" est excellent."),
            ])
            .justify(),
        ));
        items.push(Item::Para(p(vec![b("Vos priorités stratégiques :")])));
        let mut k = 0;
        let mut num = |runs: Vec<Run>| {
            k += 1;
            let mut v = vec![r(format!("{k}. "))];
            v.extend(runs);
            Item::Para(p(v).justify())
        };
        let noc_line = |who: &str, n: &Option<facts::Noc>, job: &str| -> Para {
            let mut v = vec![
                r(format!("Pour {who}")),
                r(if job.is_empty() {
                    String::new()
                } else {
                    format!(" ({job})")
                }),
                r(" : le rôle relève de la "),
            ];
            match n {
                Some(n) if n.validated => v.push(b(format!("CNP {} (FEER {})", n.code, n.feer))),
                Some(n) => v.push(todo(format!("CNP {} (FEER {}) à valider", n.code, n.feer))),
                None => v.push(todo("CNP [à déterminer]")),
            }
            v.push(r("."));
            Para::new(v).bullet(2)
        };
        items.push(num(vec![
            b("Valider précisément vos codes CNP (Étape cruciale) : "),
            r("Le choix de votre classification nationale des professions (CNP) est "),
            b("la pierre angulaire de tout votre dossier"),
            r(". Selon la réalité exacte de vos tâches quotidiennes :"),
        ]));
        items.push(Item::Para(noc_line(
            "le candidat principal",
            &f.noc,
            f.a("job"),
        )));
        if f.married {
            items.push(Item::Para(noc_line(
                "le conjoint",
                &f.spouse_noc,
                f.a("spouse_job"),
            )));
        }
        items.push(Item::Para(
            p(vec![
                b("Pourquoi c'est déterminant ?"),
                r(" La CNP conditionne directement votre niveau de FEER, votre éligibilité à la Mobilité Francophone, le calcul de vos points dans Entrée Express et l'accès au Permis de Travail Ouvert pour le conjoint. Une mauvaise correspondance peut bloquer un dossier."),
            ])
            .justify(),
        ));
        items.push(num(vec![
            b("Miser sur la Mobilité Francophone (Option N°1) : "),
            r("C'est la voie la plus simple et rapide. En décrochant une offre d'emploi hors Québec dans une CNP qualifiée (FEER 0, 1, 2 ou 3), vous bénéficiez d'un permis de travail temporaire dispensé d'EIMT. Cela vous permettra de vous installer, de cumuler de l'expérience canadienne et d'enclencher votre Résidence Permanente (via Entrée Express ou des volets provinciaux comme celui du Nouveau-Brunswick)."),
        ]));
        if f.funds == Funds::NotEnough {
            items.push(num(vec![
                b("Consolider votre autonomie financière : "),
                r("des fonds d’établissement disponibles et non grevés de dettes sont exigés pour la RP (attestation bancaire de 6 mois)."),
            ]));
        }
        if f.under_36() {
            let both = f.married && f.spouse_age.is_some_and(|a| a <= 35);
            items.push(num(vec![
                b("Inscrivez-vous au PVT : "),
                r(format!(
                    "Dès la réouverture du bassin (vers mi-décembre), tentez votre chance{} afin de maximiser vos opportunités (tirage au sort offrant un permis de travail ouvert), sous réserve de votre nationalité.",
                    if both { " tous les deux" } else { "" }
                )),
            ]));
        }
        if f.quebec {
            items.push(num(vec![
                b("Élargir vos horizons hors Québec : "),
                r("Au Québec, les règles actuelles d'immigration sont très restreintes et les délais sont particulièrement longs, même avec une offre d'emploi "),
                r("(si vous tenez absolument au Québec, la piste du permis Jeunes Professionnels pourra être analysée).").italic(),
            ]));
        }
        if f.relative {
            items.push(num(vec![
                b("Votre proche au Canada : "),
                r("un frère ou une sœur citoyen(ne) ou résident(e) permanent(e) donne des points à l’Entrée express ; précisez le lien et le statut."),
            ]));
        }
    }
    if let Some(note) = f.lawyer.note.as_deref().filter(|n| !n.trim().is_empty()) {
        items.push(Item::Para(p(vec![r(note.trim())]).justify()));
    }
}

fn closing(items: &mut Vec<Item>) {
    items.push(Item::Para(
        p(vec![
            r("🚀 PROCHAINE ÉTAPE RECOMMANDÉE : VOTRE CONSULTATION STRATÉGIQUE (1 HEURE)")
                .color(BILAN)
                .size(24),
        ])
        .after(160),
    ));
    items.push(Item::Para(
        p(vec![
            r("Cet Audit vous a donné une vision globale, mais la "),
            b("Consultation Stratégique"),
            r(" d’une heure est le moment où nous transformons ces pistes en un plan d'action concret et sur mesure."),
        ])
        .justify(),
    ));
    items.push(Item::Para(p(vec![r(
        "Pendant 60 minutes en tête-à-tête, nous allons ensemble :",
    )])));
    for (head, text) in [
        (
            "Naviguer en direct sur les sites officiels et déterminer le CNP exact :",
            " Nous explorons les portails d'immigration ensemble pour que vous compreniez exactement où et comment les démarches vont se dérouler.",
        ),
        (
            "Calculer et optimiser votre score en temps réel :",
            " Nous faisons des simulations de points en direct afin d'identifier immédiatement les leviers pour augmenter votre score.",
        ),
        (
            "Cibler les programmes les plus adaptés :",
            " Nous comparons les options (volets provinciaux, Entrée express, etc.) selon votre profil exact pour retenir la stratégie la plus rapide et sécurisée.",
        ),
        (
            "Anticiper les pièges et problématiques :",
            " Nous analysons les éventuels points bloquants de votre dossier (antécédents, expérience, diplômes) pour les désamorcer avant le dépôt. Vous repartez avec les meilleures pratiques pour préparer vos pièces justificatives et éviter les erreurs qui retardent ou font rejeter une demande.",
        ),
        (
            "Partager des expériences sur les provinces :",
            " Après 6 ans passés au Québec et désormais installée en Alberta, je connais parfaitement les réalités du terrain à travers le Canada. Nous faisons le point sur les spécificités locales afin que vous puissiez choisir la meilleure destination et préparer votre intégration en toute sérénité.",
        ),
        (
            "Mise en relation avec des recruteurs de mon réseau :",
            " Je travaille conjointement avec une équipe de recruteur très intéressée par des profils techniques (Construction, soudure, électricien, etc.)",
        ),
    ] {
        items.push(Item::Para(
            bullet(vec![r(head).small_caps().color(BILAN), r(text)]).justify(),
        ));
    }
    items.push(Item::Para(
        p(vec![r("C'était un plaisir de vous accompagner et de vous guider dans cette étape. Un grand merci pour votre confiance, et au plaisir d'échanger très bientôt !")])
            .justify()
            .after(160),
    ));
    items.push(Item::Para(p(vec![r("Vanessa POUGHON – Avocate")])));
    items.push(Item::Para(p(vec![todo("[signature]")]).after(240)));
    items.push(Item::PageBreak);
    items.push(Item::Para(p(vec![b("ANNEXE")]).center()));
    items.push(Item::Para(
        p(vec![
            r("1 - Recherche sur le site guichet emploi – cocher la case EIMT demandée, ça démontre des employeurs prêts à embaucher des candidats internationaux "),
            b("(même si pour la mobilité francophone, vous n’en avez pas besoin !)"),
            r(" (surlignée en jaune sur la photo) "),
            r("https://www.guichetemplois.gc.ca/trouverunemploi").link("https://www.guichetemplois.gc.ca/trouverunemploi"),
        ])
        .justify(),
    ));
    items.push(Item::Para(p(vec![todo(
        "[capture d’écran du Guichet-Emplois]",
    )])));
}

#[cfg(test)]
mod tests;
