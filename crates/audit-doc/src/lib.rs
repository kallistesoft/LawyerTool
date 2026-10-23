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
//!
//! Layout: a modern, web-like design (cover band, numbered sections with an
//! accent bar, status badges, cards, a roadmap timeline, footer with page
//! numbers). Sizes, centred cover and the rule under section titles follow
//! her corrected audit of 2026-10-23 (body text 11 pt); names, capitals,
//! wording and the annex screenshot follow her audit « CHAPUIS » of the same
//! day: « Prénom NOM » on the cover, « NOM PRÉNOM » in the footer.

mod cnp;
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

/// Public Guichet-Emplois search page, « EIMT » filter highlighted (annex).
const GUICHET_EMPLOIS: &[u8] = include_bytes!("../assets/guichet-emplois.png");

const FONT: &str = "Aptos";
/// Body text size (half-points): 11 pt.
const BODY_SIZE: u16 = 22;
// Palette.
const NAVY: &str = "0E2A3B";
const NAVY_SOFT: &str = "9FB3C8";
const TEAL: &str = "0F766E";
const GOLD: &str = "D4A72C";
const INK: &str = "1F2937";
const MUTED: &str = "6B7280";
const LINE: &str = "E5E7EB";
const SOFT: &str = "F8FAFC";
const CARD_BLUE: &str = "EEF4F8";

/// Status badge: light tint and text colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Green,
    Olive,
    Amber,
    Red,
}

impl Tone {
    fn tint(self) -> &'static str {
        match self {
            Tone::Green => "E7F6EC",
            Tone::Olive => "F3F3DE",
            Tone::Amber => "FFF3DC",
            Tone::Red => "FDECEC",
        }
    }
    fn ink(self) -> &'static str {
        match self {
            Tone::Green => "1E7B45",
            Tone::Olive => "6B6B1F",
            Tone::Amber => "A15C00",
            Tone::Red => "B42318",
        }
    }
}

fn r(t: impl Into<String>) -> Run {
    Run::new(t)
}

fn b(t: impl Into<String>) -> Run {
    Run::new(t).bold()
}

fn muted(t: impl Into<String>) -> Run {
    Run::new(t).color(MUTED)
}

/// A blank or a value the lawyer must check (yellow highlight).
/// « 63200 ou 62200 » for the synthesis table.
fn codes(choices: &[facts::Choice]) -> String {
    choices
        .iter()
        .map(|c| c.code)
        .collect::<Vec<_>>()
        .join(" ou ")
}

fn todo(t: impl Into<String>) -> Run {
    Run::new(t).bold().highlight()
}

fn p(runs: Vec<Run>) -> Para {
    Para::new(runs)
}

fn bullet(runs: Vec<Run>) -> Para {
    Para::new(runs).mark("•", TEAL)
}

fn label(t: &str) -> Run {
    Run::new(t).bold().small_caps().color(MUTED).spacing(10)
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

/// Last name and given names. The form asks « nom et prénom », so the last
/// name is the words written in capitals (« LE BAYEC Argentina »), else the
/// first word.
fn split_name(full_name: &str) -> (Vec<&str>, Vec<&str>) {
    let words: Vec<&str> = full_name.split_whitespace().collect();
    let is_caps = |w: &&str| {
        w.chars().filter(|c| c.is_alphabetic()).count() > 1 && w.chars().all(|c| !c.is_lowercase())
    };
    let (last, given): (Vec<&str>, Vec<&str>) = words.iter().copied().partition(is_caps);
    if last.is_empty() {
        (
            words.iter().take(1).copied().collect(),
            words.iter().skip(1).copied().collect(),
        )
    } else {
        (last, given)
    }
}

/// « Mallory CHAPUIS » (cover) and « CHAPUIS MALLORY » (footer).
fn display_names(full_name: &str) -> (String, String) {
    let (last, given) = split_name(full_name);
    let last = last.join(" ").to_uppercase();
    let given = given.join(" ");
    let cover = format!("{given} {last}").trim().to_owned();
    let footer = format!("{last} {}", given.to_uppercase()).trim().to_owned();
    (cover, footer)
}

/// `Audit_<NOM>.docx`, the last name as in [`split_name`].
pub fn file_name(full_name: &str) -> String {
    let pick = split_name(full_name).0.join("-");
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

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}

/// Builds the audit for one respondent.
pub fn build(full_name: &str, form: &FormRecord, ctx: &Context) -> Doc {
    let f = Facts::of(form);
    let (name, footer_name) = display_names(full_name);
    let mut d = Sections::default();
    cover(&mut d.items, &name, ctx);
    d.section("Synthèse du profil du candidat");
    synthesis(&mut d.items, &f, ctx);
    d.items.push(Item::PageBreak);
    d.items.push(card(
        vec![
            p(vec![label("Remarque préalable")]),
            p(vec![
                r("L’analyse contenue dans cet audit repose sur les ").color(MUTED),
                b("informations communiquées par le client à la date du questionnaire.").color(INK),
                r(" Les politiques d'immigration provinciales et fédérales étant sujettes à des modifications fréquentes et sans préavis par arrêté ministériel ou instruction générale, les stratégies préconisées dans ce document constituent une évaluation de faisabilité à un instant T et ne sauraient garantir la délivrance d'un visa, d'un permis ou d'un statut de résident permanent, décision qui relève du pouvoir souverain exclusif d'IRCC et du MIFI.").color(MUTED),
            ])
            .justify(),
        ],
        SOFT,
        MUTED,
    ));
    d.section("Faisabilité des voies d’immigration");
    feasibility(&mut d.items, &f, ctx);
    if f.married {
        d.items.push(Item::PageBreak);
        d.section("Audit sur-mesure & impact stratégique du conjoint");
        spouse(&mut d.items, &f);
    }
    d.items.push(Item::PageBreak);
    d.section("Feuille de route & plan d’action");
    roadmap(&mut d.items, &f);
    d.section("Bilan de votre audit");
    bilan(&mut d.items, &f);
    closing(&mut d);
    Doc {
        items: d.items,
        footer: Some(format!(
            "POUGHON Immigration · Audit éclair d’éligibilité · {footer_name}"
        )),
        font: Some(FONT),
        size: Some(BODY_SIZE),
    }
}

/// Items with numbered sections (« 01 », « 02 »…).
#[derive(Default)]
struct Sections {
    items: Vec<Item>,
    n: u8,
}

impl Sections {
    fn section(&mut self, title: &str) {
        self.n += 1;
        self.items.push(Item::Para(
            p(vec![
                b(format!("{:02}  ", self.n)).color(TEAL).size(28),
                b(title).color(INK).size(28),
            ])
            .bar(TEAL, 24)
            .rule("auto")
            .before(320)
            .after(180)
            .keep_next(),
        ));
    }
}

/// One rounded-looking card: tinted background and a coloured left strip.
fn card(paras: Vec<Para>, fill: &'static str, strip: &'static str) -> Item {
    Item::Grid {
        widths: vec![1],
        header: None,
        rows: vec![vec![Cell::new(paras).fill(fill).strip(strip, 24).top()]],
        lines: None,
        gap: 0,
    }
}

fn cover(items: &mut Vec<Item>, name: &str, ctx: &Context) {
    let cell = Cell::new(vec![
        p(vec![
            b("POUGHON IMMIGRATION")
                .color("FFFFFF")
                .size(22)
                .spacing(40),
        ])
        .before(200)
        .center(),
        p(vec![
            r("Cabinet d’avocat · Membre du Barreau du Québec – n° 381467-0")
                .color(NAVY_SOFT)
                .size(16),
        ])
        .after(520)
        .center(),
        p(vec![
            b(format!("CANADA · IMMIGRATION {}", &ctx.today.as_str()[..4]))
                .color(GOLD)
                .size(18)
                .spacing(30),
        ])
        .center(),
        p(vec![
            b("Audit éclair d’éligibilité").color("FFFFFF").size(52),
        ])
        .after(120)
        .center(),
        p(vec![b(name).color(GOLD).size(32).spacing(10)]).center(),
        p(vec![
            r(format!("Évaluation du {}", long_date(&ctx.today)))
                .color(NAVY_SOFT)
                .size(18),
        ])
        .after(200)
        .center(),
    ])
    .fill(NAVY);
    items.push(Item::Grid {
        widths: vec![1],
        header: None,
        rows: vec![vec![cell]],
        lines: None,
        gap: 0,
    });
}

/// A synthesis row: label, profile, and the analysis as a status badge.
fn row(label_text: &str, eval: Vec<Run>, tone: Tone, impact: Vec<Run>) -> Vec<Cell> {
    let mut badge = vec![r("●  ").color(tone.ink())];
    badge.extend(impact.into_iter().map(|x| {
        if x.color.is_none() {
            x.color(tone.ink()).bold()
        } else {
            x
        }
    }));
    vec![
        Cell::runs(vec![label(label_text)]),
        Cell::runs(eval),
        Cell::runs(badge).fill(tone.tint()),
    ]
}

fn synthesis(items: &mut Vec<Item>, f: &Facts, ctx: &Context) {
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
        "Âge & foyer",
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
    rows.push(row("Nationalité", eval, tone, vec![r(impact)]));

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
        muted(format!(" ({eca})")),
    ];
    let (tone, impact) = if f.diploma_in_progress {
        (Tone::Olive, "À confirmer")
    } else {
        (Tone::Green, "Exigence minimale acquise")
    };
    rows.push(row("Diplômes", eval, tone, vec![r(impact)]));

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
    match &f.noc {
        // A known code reads as the profile itself, not as a note.
        Some(n) if n.title.is_some() => eval.push(r(format!(" (CNP {})", n.code))),
        n => {
            eval.push(muted(" (CNP "));
            eval.push(match n {
                Some(n) => todo(format!("{} introuvable dans la CNP 2021", n.code)),
                None if !f.noc_choices.is_empty() => {
                    todo(format!("à choisir : {}", codes(&f.noc_choices)))
                }
                None => todo("à valider"),
            });
            eval.push(muted(")"));
        }
    }
    // The FEER of the CNP; without a chosen CNP, the FEERs of the proposed ones.
    let mut feers: Vec<u8> = match &f.noc {
        Some(n) => vec![n.feer],
        None => f
            .noc_choices
            .iter()
            .map(|c| c.code.as_bytes()[1] - b'0')
            .collect(),
    };
    feers.sort_unstable();
    feers.dedup();
    let (tone, impact) = match feers.as_slice() {
        [feer] if *feer <= 1 => (
            Tone::Green,
            vec![r(format!("FEER {feer}, métier très qualifié"))],
        ),
        [feer] if *feer <= 3 => (
            Tone::Green,
            vec![r(format!("FEER {feer}, métier qualifié"))],
        ),
        [feer] => (
            Tone::Amber,
            vec![r(format!(
                "Métier peu qualifié (FEER {feer}) et non admissible à l'Entrée express"
            ))],
        ),
        [] => (Tone::Green, vec![todo("FEER à définir selon la CNP")]),
        several => {
            let list = several
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(" ou ");
            let tone = if several.iter().all(|x| *x <= 3) {
                Tone::Green
            } else {
                Tone::Amber
            };
            (
                tone,
                vec![todo(format!("FEER {list} selon la CNP retenue"))],
            )
        }
    };
    rows.push(row("Profession", eval, tone, impact));

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
        (Tone::Amber, "Français à renforcer (NCLC 7 visé)")
    };
    rows.push(row("Langues", eval, tone, vec![r(impact)]));

    // Fonds.
    let household = 1 + u8::from(f.married);
    let plural = if household > 1 { "s" } else { "" };
    let needed = funds_needed(ctx, household).map_or_else(|| "[montant]".into(), money);
    let (eval, tone, impact) = match f.funds {
        Funds::Enough(n, x) => (
            "Conforme".to_owned(),
            Tone::Green,
            format!("Au moins {} $ CAD d'économies pour {n}", money(x)),
        ),
        Funds::NotEnough => (
            "Non conforme".to_owned(),
            Tone::Red,
            format!(
                "Au moins {needed} $ CAD d'économies requis pour {household} personne{plural}. Attention : Les fonds doivent être disponibles et non grevés de dettes. Attestation bancaire de 6 mois à justifier."
            ),
        ),
        Funds::Unknown => (
            "Non renseigné".to_owned(),
            Tone::Olive,
            format!("Minimum {needed} $ CAD requis pour {household} personne{plural} pour la RP"),
        ),
    };
    rows.push(row(
        "Fonds disponibles",
        vec![r(eval)],
        tone,
        vec![r(impact)],
    ));

    // Province.
    let prov = f.a("province");
    let (tone, impact) = if f.quebec_only {
        (Tone::Amber, "Immigration complexe et longue")
    } else {
        (Tone::Green, "Atout stratégique majeur")
    };
    rows.push(row(
        "Province visée",
        vec![r(if prov.is_empty() {
            "À préciser"
        } else {
            prov
        })],
        tone,
        vec![r(impact)],
    ));

    items.push(Item::Grid {
        widths: vec![22, 42, 36],
        header: Some(vec![
            Cell::runs(vec![label("Critère")]),
            Cell::runs(vec![label("Évaluation réelle du profil")]),
            Cell::runs(vec![label("Analyse de l’impact")]),
        ]),
        rows,
        lines: Some(LINE),
        gap: 0,
    });
}

fn english_bullet(f: &Facts) -> Vec<Run> {
    match f.english {
        Some(Level::Bilingual | Level::Native) => vec![
            b("Votre anglais bilingue est un très gros atout"),
            r(" dans tout le Canada."),
        ],
        Some(Level::Intermediate) => {
            let mut v = vec![b("Votre anglais intermédiaire est un atout.")];
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
                b("Votre anglais débutant doit être amélioré"),
                r(format!(" {where_}.")),
            ]
        }
    }
}

/// One route card: level on the left (tinted, coloured strip), analysis on the right.
fn route(
    title: &str,
    level: &str,
    pct: &str,
    note: &str,
    tone: Tone,
    analysis: Vec<Para>,
) -> Vec<Cell> {
    vec![
        Cell::new(vec![
            p(vec![label(title).size(17)]),
            p(vec![b(level).color(tone.ink()).size(32)]).after(0),
            p(vec![b(pct).color(tone.ink()).size(22)]),
            p(vec![muted(note).size(17)]),
        ])
        .fill(tone.tint())
        .strip(tone.ink(), 36)
        .top(),
        Cell::new(analysis).fill(SOFT).top(),
    ]
}

fn feasibility(items: &mut Vec<Item>, f: &Facts, ctx: &Context) {
    let mut rows = Vec::new();
    if f.quebec {
        rows.push(route(
            "📍 Au Québec",
            "Complexe",
            "20 %",
            "Voie complexe sans expérience locale et sans offre d’emploi",
            Tone::Amber,
            vec![
                p(vec![
                    b("PSTQ / PEQ : "),
                    r("Resserrement MIFI. Priorité exclusive aux diplômés/travailleurs sur place."),
                ])
                .mark("✕", Tone::Red.ink()),
                p(vec![
                    b("Permis temporaire : "),
                    r("Délais importants, EIMT/CAQ lourde pour l'employeur."),
                ])
                .mark("✕", Tone::Red.ink()),
            ],
        ));
    }
    let cutoff = ctx.french_cutoff.unwrap_or(DEFAULT_FRENCH_CUTOFF);
    let (level, pct, note, tone) = if f.low_skilled() {
        ("Modéré", "50 %", "Via la mobilité francophone", Tone::Amber)
    } else {
        ("Élevé", "80 %", "Voie Royale Recommandée", Tone::Green)
    };
    let mut analysis = vec![
        p(vec![
            b("Mobilité Francophone (Permis C16 - Prioritaire) : "),
            r("Aucune limite d'âge, dispense d'EIMT pour l'employeur."),
        ])
        .mark("1", TEAL),
        p(if f.low_skilled() {
            vec![
                b("Entrée Express : "),
                r(format!(
                    "après de l’expérience dans un métier qualifié (FEER 0 à 3) ; tirages cibles francophones (~{cutoff} pts)."
                )),
            ]
        } else {
            vec![
                b("Entrée Express : "),
                r(format!("Tirages cibles Francophones très favorables (~{cutoff} pts).")),
            ]
        })
        .mark("2", TEAL),
    ];
    let nb = f.english.is_none_or(|l| l < Level::Bilingual) || f.nb;
    if nb {
        analysis.push(
            p(vec![
                b("Province officiellement bilingue, le Nouveau-Brunswick (NB) : "),
                r("Transition douce pour la langue et accès rapide à la RP après 1 an. D’autres provinces aident les francophones à s’implanter avec la résidence permanente à la clef. Le programme PICA (incluant le NB) est également très pertinent à creuser, avec beaucoup d’employeurs participants."),
            ])
            .mark("3", TEAL),
        );
    }
    analysis.push(p(english_bullet(f)).mark("🗣", TEAL));
    rows.push(route(
        "📍 Hors Québec · fédéral & provinces",
        level,
        pct,
        note,
        tone,
        analysis,
    ));
    items.push(Item::Grid {
        widths: vec![30, 70],
        header: None,
        rows,
        lines: None,
        gap: 160,
    });
}

/// Progress as a bar of 10 segments with its percentage.
fn progress(caption: &str, pct: u8, tone: Tone, word: &str) -> Cell {
    let filled = usize::from(pct) / 10;
    Cell::new(vec![
        p(vec![muted(caption).size(16)]).after(20),
        p(vec![
            r("━".repeat(filled)).color(TEAL).size(16),
            r("━".repeat(10 - filled)).color("D7DEE6").size(16),
        ])
        .after(0),
        p(vec![b(format!("{pct} %")).size(17)]).after(40),
        p(vec![b(format!("●  {word}")).color(tone.ink()).size(17)]),
    ])
}

fn spouse(items: &mut Vec<Item>, f: &Facts) {
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
    items.push(card(
        vec![p(vec![
            label("Conjoint"),
            r("   "),
            r(format!("{}.", intro.join(" · "))),
        ])],
        CARD_BLUE,
        NAVY,
    ));

    let job = if f.a("job").is_empty() {
        "Votre métier"
    } else {
        f.a("job")
    };
    let crit = |t: &str| Cell::runs(vec![b(t.to_uppercase()).color(INK)]);
    let mut rows = Vec::new();
    rows.push(match &f.noc {
        Some(n) if n.qualified() => vec![
            crit("Permis de travail ouvert (PTO)"),
            Cell::runs(vec![b(job)]),
            progress("Potentiel PTO", 100, Tone::Green, "DÉCISIF"),
            Cell::runs(vec![
                r(format!("Un métier FEER {} donne droit à un ", n.feer)),
                b("Permis de Travail Ouvert pour le Conjoint"),
                r("."),
            ]),
        ],
        Some(n) => vec![
            crit("Permis de travail ouvert (PTO)"),
            Cell::runs(vec![
                b(format!("{job} : ")),
                r(format!("poste peu qualifié (FEER {}).", n.feer)),
            ]),
            progress("Potentiel PTO", 0, Tone::Red, "NON ADMISSIBLE"),
            Cell::runs(vec![
                r("Le conjoint n’obtient pas de permis ouvert avec un métier FEER 4 ou 5.")
                    .color(Tone::Red.ink()),
            ]),
        ],
        None => vec![
            crit("Permis de travail ouvert (PTO)"),
            Cell::runs(vec![
                b(format!("{job} : ")),
                todo("FEER à définir selon la CNP"),
            ]),
            progress("Potentiel PTO", 25, Tone::Amber, "DÉCISIF"),
            Cell::runs(vec![
                r("À définir précisément ; un métier FEER 0 à 3 donne droit à un "),
                b("Permis de Travail Ouvert pour le Conjoint"),
                r("."),
            ]),
        ],
    });
    rows.push(vec![
        crit("Français du conjoint"),
        Cell::runs(if f.spouse_test {
            vec![r("Test de langue fait : attendre le résultat.")]
        } else {
            vec![
                r("Test de "),
                b("français"),
                r(" à planifier pour viser B2 (NCLC 7+)."),
            ]
        }),
        progress("Niveau actuel", 25, Tone::Amber, "DÉCISIF"),
        Cell::runs(vec![r("Un résultat NCLC 7+ apporte un max de points.")]),
    ]);
    let dip = if f.a("spouse_education").is_empty() {
        "À préciser"
    } else {
        f.a("spouse_education")
    };
    rows.push(match f.spouse_has_bac {
        Some(false) => vec![
            crit("Équivalence diplôme du conjoint (ÉDE)"),
            Cell::runs(vec![r(dip)]),
            progress("Utilité ÉDE", 0, Tone::Amber, "PAS UTILE"),
            Cell::runs(vec![
                r("N'apporte pas de point au demandeur principal.").color(Tone::Red.ink()),
            ]),
        ],
        _ => vec![
            crit("Équivalence diplôme du conjoint (ÉDE)"),
            Cell::runs(vec![r(dip)]),
            progress("Utilité ÉDE", 100, Tone::Green, "DÉCISIF"),
            Cell::runs(vec![r("Apporte des points au demandeur principal.")]),
        ],
    });
    let (status, pct, word, tone) = match f.funds {
        Funds::Enough(..) => ("Conforme", 100, "ACQUIS", Tone::Green),
        Funds::NotEnough => ("Non conforme", 0, "À CONSOLIDER", Tone::Red),
        Funds::Unknown => ("Non renseigné", 0, "INCONNU", Tone::Amber),
    };
    rows.push(vec![
        crit("Autonomie financière"),
        Cell::runs(vec![r(status)]),
        progress("Fonds", pct, tone, word),
        Cell::new(vec![
            p(vec![r("Un seuil financier minimal est exigé pour la RP.")]),
            p(vec![
                b("Attention"),
                r(" : les fonds doivent être disponibles et non grevés de dettes. Attestation bancaire de 6 mois à justifier."),
            ]),
        ]),
    ]);
    items.push(Item::Grid {
        widths: vec![20, 27, 23, 30],
        header: Some(vec![
            Cell::runs(vec![label("Critère")]),
            Cell::runs(vec![label("Situation du candidat")]),
            Cell::runs(vec![label("Avancement & impact")]),
            Cell::runs(vec![label("Analyse stratégique")]),
        ]),
        rows,
        lines: Some(LINE),
        gap: 0,
    });
}

struct Step {
    title: &'static str,
    actions: Vec<Para>,
    who: String,
    delay: String,
    result: String,
}

fn roadmap(items: &mut Vec<Item>, f: &Facts) {
    let who = if f.married {
        "Requérant + conjoint"
    } else {
        "Requérant"
    };
    let mut steps: Vec<Step> = Vec::new();
    let preliminary = f.low_skilled() && f.under_36() && !f.married;
    if preliminary {
        steps.push(Step {
            title: "Préliminaire · PVT",
            actions: vec![bullet(vec![
                r("Vous pourriez vous inscrire au "),
                b("PVT (Programme EIC)"),
                r(". C'est un tirage au sort mais c'est un programme qui vous permettrait d'obtenir un permis de travail ouvert (PTO) dans TOUTES les provinces du Canada."),
            ])],
            who: "Requérant".into(),
            delay: "très aléatoire ; inscription valable un an, renouvelable jusqu'aux 35 ans".into(),
            result: "Permis de travail ouvert de 2 ans".into(),
        });
    }
    let test = if f.married {
        "Test d’anglais pour les deux,"
    } else {
        "Test d’anglais"
    };
    let english_action = match f.english {
        Some(Level::Bilingual | Level::Native) => vec![b(test), r(" opportun")],
        Some(Level::Intermediate) => vec![b(test), r(" opportun si niveau suffisant")],
        _ => vec![b(test), r(" non opportun")],
    };
    if f.french_test == FrenchTest::Done {
        steps.push(Step {
            title: "Test d’anglais (NCLC 5+)",
            actions: vec![
                bullet(vec![r(
                    "Test de français réalisé : joindre l’attestation officielle.",
                )]),
                bullet(english_action),
                bullet(vec![r("Au choix : CELPIP / IELTS / PTE Core.")]),
            ],
            who: who.into(),
            delay: "1-2 mois".into(),
            result: "Attestations officielles de langue".into(),
        });
    } else {
        steps.push(Step {
            title: "Test de français (NCLC 9+) & test d’anglais (NCLC 5+)",
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
                    b("Obligatoire"),
                    r(" pour justifier de la Mobilité Francophone (NCLC 5 requis)."),
                ]),
                bullet(english_action),
            ],
            who: who.into(),
            delay: "1-2 mois".into(),
            result: "Attestation officielle NCLC 9+ en français".into(),
        });
    }
    let regulated: Vec<Run> = match &f.lawyer.regulated {
        Some(t) if !t.trim().is_empty() => vec![b(t.trim().to_owned())],
        _ => vec![todo("[réglementée ou non : à vérifier selon la CNP]")],
    };
    let eca = vec![
        bullet(vec![
            r("Lancement de "),
            b("l'ÉDE"),
            r(" (WES/ICAS) pour valoriser le diplôme auprès des employeurs "),
            b("et si vous tentez l’Entrée express."),
        ]),
        bullet(
            [
                vec![r("Attention aux métiers réglementés (près de 20 % au Canada) : votre profession ")],
                regulated,
                vec![r(".")],
            ]
            .concat(),
        ),
    ];
    steps.push(Step {
        title: "Évaluation des diplômes",
        actions: eca,
        who: format!("{who} et avocat si besoin"),
        delay: "2-3 mois".into(),
        result: "Rapport d'ÉDE officiel (pas important pour la mobilité francophone mais indispensable pour l’Entrée express)".into(),
    });
    steps.push(Step {
        title: if f.quebec {
            "Démarchage & permis C16 (mobilité francophone, donc hors Québec)"
        } else {
            "Démarchage & permis C16"
        },
        actions: vec![
            bullet(vec![
                r("Préparation d'un "),
                b("CV canadien").link("https://www.guichetemplois.gc.ca/rapports-tendances/conseils-carriere/rediger-cv"),
                r("."),
            ]),
            bullet(vec![
                r("Candidatures "),
                b("ciblées"),
                r(" auprès des entreprises dans votre domaine (voir l’annexe pour la recherche d’emploi sur le Guichet-Emplois)."),
            ]),
            bullet(vec![r("Mise en avant expresse de la "), b("« Dispense d'EIMT »"), r(" (code C16).")]),
        ],
        who: "Client + avocat si besoin".into(),
        delay: "2-4 mois".into(),
        result: "Offre d'emploi validée + permis C16 délivré".into(),
    });
    if f.low_skilled() {
        steps.push(Step {
            title: "Création d’expérience & RP",
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
            delay: "12 à 36 mois selon la procédure envisagée".into(),
            result: "Statut de Résident Permanent (RP) obtenu".into(),
        });
    } else {
        steps.push(Step {
            title: "Installation & RP",
            actions: vec![
                bullet(vec![r("Arrivée au Canada, "), b("démarches"), r(" d'installation (NAS, logement).")]),
                bullet(vec![r("Prise de poste.")]),
                bullet(vec![
                    r("Dépôt de la demande de "),
                    b("Résidence Permanente"),
                    r(" (Entrée Express Francophone ou programme provincial), "),
                    b("après vérification du score SCG et des conditions réunies pour l’Entrée express").italic().color(Tone::Red.ink()),
                    r(if f.quebec { ", seulement hors Québec." } else { "." }),
                ]),
            ],
            who: "Client + avocat si besoin".into(),
            delay: "8 à 30 mois selon la procédure envisagée".into(),
            result: "Statut de Résident Permanent (RP) obtenu".into(),
        });
    }
    let rows = steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let number = if preliminary && i == 0 {
                "★".to_owned()
            } else {
                format!("{}", i + 1 - usize::from(preliminary))
            };
            let mut detail = s.actions;
            detail.push(
                p(vec![
                    muted(format!("👤 {}", s.who)),
                    muted(format!("     ⏱ {}", s.delay)),
                ])
                .before(80)
                .after(20),
            );
            detail.push(p(vec![b("→  ").color(TEAL), b(s.result).color(TEAL)]));
            vec![
                Cell::new(vec![
                    p(vec![b(number).color(TEAL)]).after(0),
                    p(vec![b(s.title.to_uppercase()).color(INK)]),
                ])
                .top(),
                Cell::new(detail).top(),
            ]
        })
        .collect();
    items.push(Item::Grid {
        widths: vec![24, 76],
        header: None,
        rows,
        lines: Some(LINE),
        gap: 0,
    });
}

fn bilan(items: &mut Vec<Item>, f: &Facts) {
    let concerns = f.concerns();
    if !concerns.is_empty() {
        items.push(Item::Para(p(vec![
            todo("Point à examiner avant envoi (admissibilité) : "),
            todo(concerns.join(" ; ")),
        ])));
    }
    let mut paras = Vec::new();
    let fill;
    if f.low_skilled() {
        fill = Tone::Amber.tint();
        paras.push(
            p(vec![
                b("🥇 De l'expérience à la mobilisation : votre plan d'action")
                    .color(INK)
                    .size(28),
            ])
            .after(160),
        );
        paras.push(
            p(vec![
                r("Soyons transparents : votre métier "),
                b("n'est pas assez qualifié"),
                r(" pour tenter l'Entrée express (résidence permanente) dès maintenant. En revanche, il est tout à fait envisageable de "),
                b("monter en grade"),
                r(" dans votre métier et gagner en responsabilité afin d'atteindre de l'ancienneté dans un métier qualifié en "),
                b("FEER 3"),
                r(" a minima."),
            ])
            .justify(),
        );
        paras.push(
            p(vec![
                r("Votre priorité absolue doit être la "),
                b("recherche active"),
                r(" d'un employeur canadien et passer par une mobilité francophone (sans EIMT) dès à présent."),
            ])
            .justify(),
        );
        if f.english.is_some_and(|l| l >= Level::Bilingual) {
            paras.push(p(vec![r(
                "Votre niveau d'anglais est également un incroyable atout.",
            )]));
        }
        if f.relative {
            paras.push(p(vec![r("Votre proche au Canada (citoyen ou résident permanent) pourra faciliter votre installation : précisez le lien (seul un frère ou une sœur donne des points à l’Entrée express).")]));
        }
        paras.push(
            p(vec![r("Le chemin demandera de la méthode, de l'énergie et une prospection ciblée, mais c'est une stratégie parfaitement viable ! Il sera tout à fait possible par la suite de solliciter une résidence permanente dès les conditions réunies.")])
                .justify(),
        );
    } else {
        fill = Tone::Green.tint();
        paras.push(
            p(vec![
                b("🥇 Félicitations : un excellent potentiel !")
                    .color(INK)
                    .size(28),
            ])
            .after(160),
        );
        let mut atouts = vec!["votre profession"];
        if f.age.is_some_and(|a| a <= 35) {
            atouts.push("votre âge");
        }
        atouts.push("votre expérience professionnelle");
        if f.english.is_some_and(|l| l >= Level::Bilingual) {
            atouts.push("votre bilinguisme");
        }
        paras.push(
            p(vec![
                r(format!(
                    "Votre profil rassemble des atouts majeurs ({}). Votre potentiel de réussite pour immigrer au Canada ",
                    atouts.join(", ")
                )),
                b("hors Québec"),
                r(" est excellent."),
            ])
            .justify(),
        );
    }
    items.push(card(paras, fill, GOLD));
    if f.low_skilled() {
        note(items, f);
        return;
    }

    items.push(Item::Para(
        p(vec![label("Vos priorités stratégiques")]).before(200),
    ));
    let mut k = 0;
    let mut num = |items: &mut Vec<Item>, runs: Vec<Run>| {
        k += 1;
        let mark: &'static str = ["1", "2", "3", "4", "5", "6", "7"][(k - 1).min(6)];
        items.push(Item::Para(p(runs).mark(mark, TEAL).justify()));
    };
    let noc_line =
        |who: &str, n: &Option<facts::Noc>, choices: &[facts::Choice], job: &str| -> Para {
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
                Some(facts::Noc {
                    code,
                    feer,
                    title: Some(t),
                    ..
                }) => {
                    v.push(b(format!("CNP {code} – {t} (FEER {feer})")));
                }
                Some(n) => v.push(todo(format!(
                    "CNP {} introuvable dans la CNP 2021, à vérifier",
                    n.code
                ))),
                None if !choices.is_empty() => {
                    v.push(todo("CNP à choisir parmi : "));
                    for (i, c) in choices.iter().enumerate() {
                        let sep = if i + 1 < choices.len() { " ; " } else { "" };
                        v.push(todo(format!(
                            "{} – {} (FEER {}){sep}",
                            c.code,
                            c.title,
                            &c.code[1..2]
                        )));
                    }
                }
                None => v.push(todo("CNP [à déterminer]")),
            }
            v.push(r("."));
            Para::new(v).bullet(2)
        };
    // Once the lawyer has set a qualified CNP, choosing it is no longer a
    // priority; the French draws of Express Entry are (her CHAPUIS audit).
    let cnp_set = f
        .noc
        .as_ref()
        .is_some_and(|n| n.title.is_some() && n.qualified());
    if !cnp_set {
        num(
            items,
            vec![
                b("Valider précisément vos codes CNP (étape cruciale) : "),
                r("le choix de votre classification nationale des professions (CNP) est "),
                b("la pierre angulaire de tout votre dossier"),
                r(". Selon la réalité exacte de vos tâches quotidiennes :"),
            ],
        );
        items.push(Item::Para(noc_line(
            "le candidat principal",
            &f.noc,
            &f.noc_choices,
            f.a("job"),
        )));
        if f.married {
            items.push(Item::Para(noc_line(
                "le conjoint",
                &f.spouse_noc,
                &f.spouse_noc_choices,
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
    }
    num(
        items,
        vec![
            b("Miser sur la Mobilité Francophone (option n°1) : "),
            r(
                "c'est la voie la plus simple et rapide. En décrochant une offre d'emploi hors Québec dans une CNP qualifiée (FEER 0, 1, 2 ou 3), vous bénéficiez d'un permis de travail temporaire dispensé d'EIMT. Cela vous permettra de vous installer, de cumuler de l'expérience canadienne et d'enclencher votre Résidence Permanente (via Entrée Express ou des volets provinciaux comme celui du Nouveau-Brunswick).",
            ),
        ],
    );
    if f.funds == Funds::NotEnough {
        num(
            items,
            vec![
                b("Consolider votre autonomie financière : "),
                r(
                    "des fonds d’établissement disponibles et non grevés de dettes sont exigés pour la RP (attestation bancaire de 6 mois).",
                ),
            ],
        );
    }
    if f.under_36() {
        let both = f.married && f.spouse_age.is_some_and(|a| a <= 35);
        num(
            items,
            vec![
                b("Inscrivez-vous au PVT : "),
                r(format!(
                    "dès la réouverture du bassin (vers mi-décembre), tentez votre chance{} afin de maximiser vos opportunités (tirage au sort offrant un permis de travail ouvert), sous réserve de votre nationalité.",
                    if both { " tous les deux" } else { "" }
                )),
            ],
        );
    }
    if cnp_set && f.francophone() {
        num(
            items,
            vec![
                b(
                    "Maximiser le volet Entrée Express (Sélection basée sur les catégories – Francophones) : ",
                ),
                r(
                    "en atteignant un niveau NCLC 7+ en français (idéalement NCLC 9+), vous débloquez l'accès aux tirages ciblés pour les candidats francophones. Avec des seuils historiques très accessibles (situés généralement entre 380 et 435 points), vous devenez immédiatement extrêmement compétitifs dans le bassin fédéral sans attendre une offre d'emploi.",
                ),
            ],
        );
    }
    if f.quebec {
        num(
            items,
            vec![
                b("Élargir vos horizons hors Québec : "),
                r("au Québec, les règles actuelles d'immigration sont très restreintes et les délais sont particulièrement longs, même avec une offre d'emploi "),
                r("(si vous tenez absolument au Québec, la piste du permis Jeunes Professionnels pourra être analysée).").italic(),
            ],
        );
    }
    if f.relative {
        num(
            items,
            vec![
                b("Votre proche au Canada : "),
                r(
                    "un frère ou une sœur citoyen(ne) ou résident(e) permanent(e) donne des points à l’Entrée express ; précisez le lien et le statut.",
                ),
            ],
        );
    }
    note(items, f);
}

/// The lawyer's free paragraph, if any.
fn note(items: &mut Vec<Item>, f: &Facts) {
    if let Some(n) = f.lawyer.note.as_deref().filter(|n| !n.trim().is_empty()) {
        items.push(Item::Para(p(vec![r(n.trim())]).justify().before(120)));
    }
}

fn closing(d: &mut Sections) {
    let mut paras = vec![
        p(vec![label("Prochaine étape recommandée")]),
        p(vec![b("🚀 Votre consultation stratégique (1 heure)").color(NAVY).size(28)]).after(160),
        p(vec![
            r("Cet audit vous a donné une vision globale, mais la "),
            b("Consultation Stratégique"),
            r(" d’une heure est le moment où nous transformons ces pistes en un plan d'action concret et sur mesure. Pendant 60 minutes en tête-à-tête, nous allons ensemble :"),
        ])
        .justify(),
    ];
    for (head, text) in [
        (
            "Naviguer en direct sur les sites officiels et déterminer le CNP exact :",
            " nous explorons les portails d'immigration ensemble pour que vous compreniez exactement où et comment les démarches vont se dérouler.",
        ),
        (
            "Calculer et optimiser votre score en temps réel :",
            " nous faisons des simulations de points en direct afin d'identifier immédiatement les leviers pour augmenter votre score.",
        ),
        (
            "Cibler les programmes les plus adaptés :",
            " nous comparons les options (volets provinciaux, Entrée express, etc.) selon votre profil exact pour retenir la stratégie la plus rapide et sécurisée.",
        ),
        (
            "Anticiper les pièges et problématiques :",
            " nous analysons les éventuels points bloquants de votre dossier (antécédents, expérience, diplômes) pour les désamorcer avant le dépôt. Vous repartez avec les meilleures pratiques pour préparer vos pièces justificatives et éviter les erreurs qui retardent ou font rejeter une demande.",
        ),
        (
            "Partager des expériences sur les provinces :",
            " après 6 ans passés au Québec et désormais installée en Alberta, je connais parfaitement les réalités du terrain à travers le Canada. Nous faisons le point sur les spécificités locales afin que vous puissiez choisir la meilleure destination et préparer votre intégration en toute sérénité.",
        ),
        (
            "Mise en relation avec des recruteurs de mon réseau :",
            " je travaille conjointement avec une équipe de recruteurs très intéressée par des profils techniques (construction, soudure, électricien, etc.).",
        ),
    ] {
        paras.push(
            p(vec![b(head).color(INK), r(text)])
                .mark("✓", TEAL)
                .justify(),
        );
    }
    d.items.push(Item::Para(p(vec![]).after(120)));
    d.items.push(card(paras, CARD_BLUE, NAVY));
    d.items.push(Item::Para(
        p(vec![r("C'était un plaisir de vous accompagner et de vous guider dans cette étape. Un grand merci pour votre confiance, et au plaisir d'échanger très bientôt !")])
            .justify()
            .before(240)
            .after(200),
    ));
    d.items.push(Item::Para(p(vec![
        b("Vanessa POUGHON"),
        muted(" – Avocate"),
    ])));
    d.items.push(Item::Para(p(vec![]).after(240)));
    d.items.push(Item::PageBreak);
    d.section("Annexe");
    d.items.push(Item::Para(
        p(vec![
            r("Recherche sur le site Guichet-Emplois : cochez la case « EIMT demandée », ça démontre des employeurs prêts à embaucher des candidats internationaux "),
            b("(même si pour la mobilité francophone, vous n’en avez pas besoin !)"),
            r(" — surlignée en jaune sur la photo. "),
            r("https://www.guichetemplois.gc.ca/trouverunemploi")
                .link("https://www.guichetemplois.gc.ca/trouverunemploi")
                .color(TEAL),
        ])
        .justify(),
    ));
    d.items.push(Item::Para(p(vec![])));
    d.items.push(Item::Image {
        png: GUICHET_EMPLOIS,
        px: (987, 969),
    });
}

#[cfg(test)]
mod tests;
