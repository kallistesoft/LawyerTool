#![allow(clippy::unwrap_used)] // test code may panic

//! Identifiers hidden on purpose (or by accident) must still be caught.

use deid::{Identifiers, deidentify, skeleton};
use proptest::prelude::*;
use serde_json::json;

fn ids() -> Identifiers {
    Identifiers {
        full_name: "Hélène Tremblay".into(),
        ..Default::default()
    }
}

fn scrub(s: &str) -> String {
    deidentify(json!({ "t": s }), &ids()).payload["t"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn invisible_characters_inside_names() {
    let out = scrub("Mme Trem\u{200b}blay et H\u{feff}élène");
    assert_eq!(out, "Mme [CLIENT] et [CLIENT]");
}

#[test]
fn homoglyph_and_fullwidth_names() {
    // Cyrillic Т and е, Greek ο, full-width letters.
    for disguised in [
        "\u{422}r\u{435}mblay",
        "Tr\u{435}mbl\u{430}y",
        "Ｔｒｅｍｂｌａｙ",
    ] {
        assert_eq!(
            scrub(&format!("Client : {disguised}.")),
            "Client : [CLIENT].",
            "{disguised:?}"
        );
    }
}

#[test]
fn spaced_out_names() {
    for disguised in [
        "T r e m b l a y",
        "T.r.e.m.b.l.a.y",
        "t-r-e-m-b-l-a-y",
        "H é l è n e",
    ] {
        let out = scrub(&format!("Nom: {disguised}"));
        assert_eq!(out, "Nom: [CLIENT]", "{disguised:?}");
    }
}

#[test]
fn spelled_out_emails() {
    for e in [
        "jean.roy [at] example [dot] com",
        "jean.roy(at)example.com",
        "jean.roy at example dot com",
    ] {
        let out = scrub(&format!("écrire à {e} svp"));
        assert!(
            out.contains("[EMAIL]") && !out.contains("example"),
            "{e:?} -> {out}"
        );
    }
}

#[test]
fn unrelated_text_is_left_intact() {
    let s = "Le demandeur a 3 ans d’expérience au Canada (CNP 21231), test IELTS du 2025-06-01.";
    assert_eq!(scrub(s), s);
    // Non-Latin text that is not the name stays as written (no lossy folding).
    assert_eq!(scrub("Адрес: Москва"), "Адрес: Москва");
}

fn name_part() -> impl Strategy<Value = String> {
    "[A-Z][a-zéèêàçô]{3,9}"
}

fn disguise(word: &str, how: u8) -> String {
    match how % 5 {
        0 => word.to_owned(),
        1 => word.to_uppercase(),
        2 => word
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join("\u{200b}"),
        3 => word
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        _ => word
            .replace('e', "\u{435}")
            .replace('a', "\u{430}")
            .replace('o', "\u{43e}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// Whatever the disguise, no part of the client's name survives in the
    /// payload (checked on the normalized skeleton, case-insensitively).
    #[test]
    fn client_name_never_survives(first in name_part(), last in name_part(), before in "[a-z ,.;:()]{0,30}", after in "[a-z ,.;:()]{0,30}", how in any::<u8>(), which in any::<bool>()) {
        let ids = Identifiers { full_name: format!("{first} {last}"), ..Default::default() };
        let hidden = disguise(if which { &first } else { &last }, how);
        let payload = json!({ "note": format!("{before} {hidden} {after}"), "nested": [{ "x": format!("({hidden})") }] });
        let out = deidentify(payload, &ids).payload.to_string();
        let sk = skeleton(&out).to_lowercase().replace([' ', '.', '-', '_'], "");
        for part in [&first, &last] {
            let p = skeleton(part).to_lowercase();
            prop_assert!(!sk.contains(&p), "{part} survived in {out}");
        }
    }
}
