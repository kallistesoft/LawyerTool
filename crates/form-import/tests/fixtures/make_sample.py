#!/usr/bin/env python3
"""Writes sample-form.xlsx: an export shaped exactly like the firm's
Microsoft Forms export ("AUDIT de votre profil — IMMIGRATION AU CANADA",
quiz mode: each question has an answer, a "Points - " and a "Feedback - "
column), filled with INVENTED respondents only.

The real export holds real people's data and must never be committed.

    python3 -I crates/form-import/tests/fixtures/make_sample.py
"""
import os
import zipfile
from xml.sax.saxutils import escape

FIXED = ["ID", "Start time", "Completion time", "Email", "Name", "Total points", "Quiz feedback"]
QUESTIONS = [
    "Quel est votre nom et prénom ?",
    "Quel est votre âge et votre date de naissance ? ",
    "Quelle est votre nationalité ?",
    "Quel est votre pays de résidence actuel ? ",
    "Quel est votre statut actuellement au Canada?",
    "Adresse courriel utilisée lors de votre règlement",
    "Quelle est votre situation matrimoniale ?",
    "Quel est l'âge de votre conjoint(e) ?",
    "Quel est le statut de votre conjoint actuellement au Canada?",
    "Quelle est la nationalité de votre conjoint ?",
    "Votre conjoint(e) a-t-il/elle un diplôme d'études supérieures ?",
    "Quel est le niveau d'anglais de votre conjoint ?",
    "Quel est le métier de votre conjoint ?",
    "Combien\xa0d'ancienneté a votre conjoint dans son métier au cours des 10 dernières années ? ",
    "Votre conjoint(e) a-t-il/elle un test de langue ?",
    "Avez-vous déjà été condamné ou suspecté par la justice ?",
    "Avez-vous une maladie grave ?",
    "Quel est votre métier et domaine d'activité ? ",
    "Combien d'ancienneté avez-vous dans votre métier au cours des 10 dernières années ? ",
    "Avez-vous un diplôme canadien ?",
    "Combien d'années avez-vous déjà travaillé au Canada ?",
    "Quel est votre diplôme le plus élevé ?",
    "Avez-vous une évaluation des diplômes ?",
    "Avez-vous fais un test de français ?",
    "Avez-vous une offre d'emploi ou une promesse d'embauche d'un employeur canadien ?",
    "Pour quel poste cette offre d'emploi vous est-elle proposée ?",
    "Taille du Foyer et Fonds d'Établissement",
    "Quel est votre niveau de français ?",
    "Quel est votre niveau d'anglais?",
    "Avez-vous un frère, une sœur, un parent ou un enfant majeur qui habite au Canada (citoyen ou résident permanent) ?",
    "Quelle serait votre province de destination ?",
    "Quel est votre lien avec le Canada ?",
]
HEADERS = FIXED + [h for q in QUESTIONS for h in (q, f"Points - {q}", f"Feedback - {q}")]

FUNDS = {
    1: "1 personne : J'ai au moins 16 000 $ CAD d'économies\xa0",
    2: "2 personnes : J'ai au moins 20 000 $ CAD d'économies\xa0",
    0: "Je n'ai pas les fonds disponibles.",
}

# Invented respondents (fictional names, example.com addresses).
PEOPLE = [
    {  # single, full birth date, French test done, sibling question "Oui"
        "ID": "101", "Start time": "46313.06958333333", "Completion time": "46313.072743055556",
        "Email": "anonymous",
        QUESTIONS[0]: "TESTEUR Alix ", QUESTIONS[1]: "32 ans, 16/03/1994", QUESTIONS[2]: "Française ",
        QUESTIONS[3]: "France", QUESTIONS[4]: "Aucun statut actuellement au Canada",
        QUESTIONS[5]: "alix.testeur@example.com", QUESTIONS[6]: "Célibataire",
        QUESTIONS[15]: "Jamais condamné ou suspecté", QUESTIONS[16]: "Non ",
        QUESTIONS[17]: "Infirmière (code CNP 31301)", QUESTIONS[18]: "5 ans et plus",
        QUESTIONS[19]: "Non", QUESTIONS[20]: "0 ou moins d'1 an", QUESTIONS[21]: "Licence (bac+3)",
        QUESTIONS[22]: "Évaluation des diplômes pas encore faite", QUESTIONS[23]: "Oui",
        QUESTIONS[24]: "Non", QUESTIONS[26]: FUNDS[1], QUESTIONS[27]: "Langue maternelle",
        QUESTIONS[28]: "Intermédiaire\xa0", QUESTIONS[29]: "Oui", QUESTIONS[30]: "Nouveau-Brunswick",
        QUESTIONS[31]: "Amis",
    },
    {  # married, age only, wants Québec, some Canadian work, temporary worker
        "ID": "102", "Start time": "46314.31159722222", "Completion time": "46314.317037037035",
        "Email": "anonymous",
        QUESTIONS[0]: "Exemple Camille", QUESTIONS[1]: "38 ans", QUESTIONS[2]: "Français",
        QUESTIONS[3]: "Canada", QUESTIONS[4]: "Travailleur temporaire (EIMT) ",
        QUESTIONS[5]: "camille.exemple@example.com", QUESTIONS[6]: "Marié ou en union de fait",
        QUESTIONS[7]: "36 ans", QUESTIONS[8]: "Aucun", QUESTIONS[9]: "Française",
        QUESTIONS[10]: "3 années d'université", QUESTIONS[11]: "Débutant",
        QUESTIONS[12]: "Technicienne en méthodes", QUESTIONS[13]: "5 ans et plus", QUESTIONS[14]: "Non",
        QUESTIONS[15]: "Jamais condamné ou suspecté", QUESTIONS[16]: "Aucune",
        QUESTIONS[17]: "Cuisinier", QUESTIONS[18]: "4 ans", QUESTIONS[19]: "Non",
        QUESTIONS[20]: "3 ans", QUESTIONS[21]: "BTS", QUESTIONS[22]: "Évaluation des diplômes pas encore faite",
        QUESTIONS[23]: "Non", QUESTIONS[24]: "Non", QUESTIONS[26]: FUNDS[2],
        QUESTIONS[27]: "Langue maternelle", QUESTIONS[28]: "Bilingue", QUESTIONS[29]: "Non",
        QUESTIONS[30]: "Ontario ou Québec", QUESTIONS[31]: "Déjà travaillé.",
    },
    {  # birth date written out, no funds, health answer to examine
        "ID": "103", "Start time": "46315.04085648148", "Completion time": "46315.0434837963",
        "Email": "anonymous",
        QUESTIONS[0]: "Fictif Dominique", QUESTIONS[1]: "45 ans (née le 11 août 1981)",
        QUESTIONS[2]: "Française", QUESTIONS[3]: "france", QUESTIONS[4]: "Touriste",
        QUESTIONS[5]: "pas une adresse", QUESTIONS[6]: "Célibataire",
        QUESTIONS[15]: "Jamais condamné ou suspecté", QUESTIONS[16]: "Diabète de type 1",
        QUESTIONS[17]: "Comptable", QUESTIONS[18]: "2 ans", QUESTIONS[19]: "NON",
        QUESTIONS[20]: "0 ou moins d'1 an", QUESTIONS[21]: "Master",
        QUESTIONS[22]: "Évaluation des diplômes pas encore faite", QUESTIONS[23]: "En cours",
        QUESTIONS[24]: "Non", QUESTIONS[26]: FUNDS[0], QUESTIONS[27]: "Langue maternelle",
        QUESTIONS[28]: "Débutant", QUESTIONS[29]: "Non", QUESTIONS[30]: "Alberta",
        QUESTIONS[31]: "Aucun",
    },
    {"ID": "104", "Start time": "46315.5"},  # no name: skipped
    {},  # empty row: ignored
]


def main():
    strings, index = [], {}

    def sid(s):
        if s not in index:
            index[s] = len(strings)
            strings.append(s)
        return index[s]

    def col(i):
        s = ""
        i += 1
        while i:
            i, r = divmod(i - 1, 26)
            s = chr(65 + r) + s
        return s

    rows = [HEADERS] + [[p.get(h, "") for h in HEADERS] for p in PEOPLE]
    xml_rows = []
    for r, row in enumerate(rows, start=1):
        cells = []
        for c, v in enumerate(row):
            if v == "":
                continue
            ref = f"{col(c)}{r}"
            if r > 1 and HEADERS[c] in ("ID", "Start time", "Completion time"):
                cells.append(f'<c r="{ref}"><v>{v}</v></c>')
            else:
                cells.append(f'<c r="{ref}" t="s"><v>{sid(v)}</v></c>')
        xml_rows.append(f'<row r="{r}">{"".join(cells)}</row>')
    ns = 'xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"'
    sheet = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<worksheet {ns}><sheetData>{"".join(xml_rows)}</sheetData></worksheet>'
    sst = (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<sst {ns} count="{len(strings)}" uniqueCount="{len(strings)}">'
        + "".join(f'<si><t xml:space="preserve">{escape(s)}</t></si>' for s in strings)
        + "</sst>"
    )
    workbook = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<workbook {ns} xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>'
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "sample-form.xlsx")
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in [
            ("[Content_Types].xml", '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>'),
            ("xl/workbook.xml", workbook),
            ("xl/worksheets/sheet1.xml", sheet),
            ("xl/sharedStrings.xml", sst),
        ]:
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 21, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data)
    print(out)


main()
