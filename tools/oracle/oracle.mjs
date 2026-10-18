// Dev-only CRS oracle: runs IRCC's own calculator (scg-outil-udit.js) on the
// saved calculator page in jsdom, with no network access.
//
// Input (stdin, one JSON object per line):
//   {"id": "...", "answers": [["q1", "F"], ["q3", "M"], ...]}
// answers are applied in order, each as "select value + change event", the
// way a person fills the form. Then the form is submitted.
//
// Output (stdout, one JSON object per line):
//   {"id": "...", "core": n, "spouse": n, "skill": n, "bonus": n, "total": n}
//   or {"id": "...", "error": "..."}

import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { createHash } from "node:crypto";
import { JSDOM, VirtualConsole } from "jsdom";

const SOURCES = new URL("../../crates/rules-engine/fixtures/sources/", import.meta.url);
const PIN = {
  "verifier-note.html": "76036f8d718b7a0469943a78b74237ad30ff3abb77e01e2c50946fc61b67c348",
  "scg-outil-udit.js": "61392e6266be6d976735305982eeb72ce30296f086ebf1b5dc802a305639a65a",
};

function load(name) {
  const text = readFileSync(new URL(name, SOURCES), "utf8");
  const sha = createHash("sha256").update(text).digest("hex");
  if (sha !== PIN[name]) throw new Error(`${name}: hash ${sha} does not match pinned ${PIN[name]}`);
  return text;
}

// Keep only the form markup: no page scripts, styles or external resources.
const page = load("verifier-note.html");
const form = page.slice(page.indexOf("<main"), page.indexOf("</main>") + "</main>".length);
const calculator = load("scg-outil-udit.js");
const jquery = readFileSync(new URL("node_modules/jquery/dist/jquery.min.js", import.meta.url), "utf8");

function run(answers) {
  const virtualConsole = new VirtualConsole(); // swallow page console noise
  const dom = new JSDOM(`<!doctype html><html><body>${form}</body></html>`, {
    runScripts: "outside-only",
    virtualConsole,
  });
  const w = dom.window;
  w.eval(jquery);
  w.eval(calculator);
  w.onload?.();
  const $ = w.jQuery;

  for (const [field, value] of answers) {
    const el = $(`#${field}`);
    if (el.length !== 1) throw new Error(`no field #${field}`);
    if (el.find(`option[value="${value}"]`).length === 0) {
      const opts = el.find("option").map((_, o) => o.value).get().join(",");
      throw new Error(`#${field}: no option ${value} (have ${opts})`);
    }
    el.val(value).trigger("change");
  }
  $("#crs-form").trigger("submit");

  const html = (id) => w.document.getElementById(id)?.innerHTML ?? "";
  const num = (id, label) => {
    const m = html(id).match(new RegExp(label + "\\s*(?:</strong>)?\\s*(?:&nbsp;)?\\s*=\\s*(\\d+)"));
    if (!m) throw new Error(`result ${id}: "${label}" not found`);
    return Number(m[1]);
  };
  const out = {
    core: num("core_factors", "Sous-total - Facteurs de base/du capital humain"),
    spouse: num("spouse_factors", "Sous-total - Facteurs de l&rsquo;époux|Sous-total - Facteurs de l’époux"),
    skill: num("skill_factors", "Sous-total facteurs liés au transfert des compétences"),
    bonus: num("totals", "Sous-total des points supplémentaires"),
    total: num("totals", "Formule du système de classement somme finale"),
  };
  w.close();
  return out;
}

const rl = createInterface({ input: process.stdin });
for await (const line of rl) {
  if (!line.trim()) continue;
  let id = null;
  try {
    const req = JSON.parse(line);
    id = req.id;
    process.stdout.write(JSON.stringify({ id, ...run(req.answers) }) + "\n");
  } catch (e) {
    process.stdout.write(JSON.stringify({ id, error: String(e.message ?? e) }) + "\n");
  }
}
