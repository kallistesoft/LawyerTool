//! JavaScript interface (wasm-bindgen), used by `ui/src/lib/engine.worker.ts`.
//! JSON in, JSON out; errors are thrown as the JSON of a [`CommandError`].

use wasm_bindgen::prelude::*;

use crate::{App, CommandError};

fn js_err(e: CommandError) -> JsValue {
    JsValue::from_str(
        &serde_json::to_string(&e)
            .unwrap_or_else(|_| r#"{"code":"internal","message":"Erreur interne."}"#.into()),
    )
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<String, JsValue> {
    serde_json::to_string(v).map_err(|_| js_err(CommandError::internal()))
}

fn from_json<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, JsValue> {
    serde_json::from_str(s).map_err(|_| js_err(crate::err("invalid", "Paramètres invalides.")))
}

/// SHA-256 of a file's content (hex), to track imported form files.
#[wasm_bindgen]
pub fn fingerprint(bytes: &[u8]) -> String {
    crate::inbox::fingerprint(bytes)
}

/// Whether the inbox lists a file of this name.
#[wasm_bindgen]
pub fn accepted_name(name: &str) -> bool {
    crate::inbox::accepted_name(name)
}

/// A rendered report: metadata as JSON, bytes separately.
#[wasm_bindgen]
pub struct ExportOut {
    meta: String,
    bytes: Vec<u8>,
}

#[wasm_bindgen]
impl ExportOut {
    /// `{ name, mime, kind }`
    #[wasm_bindgen(getter)]
    pub fn meta(&self) -> String {
        self.meta.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

#[wasm_bindgen]
pub struct Engine {
    app: App,
}

#[wasm_bindgen]
impl Engine {
    /// `fast_kdf`: development test vault only (never in a production build).
    #[wasm_bindgen(constructor)]
    pub fn new(fast_kdf: bool) -> Engine {
        let kdf = if fast_kdf {
            vault_file::KdfParams::INSECURE_FAST_FOR_TESTS
        } else {
            vault_file::KdfParams::DEFAULT
        };
        Engine { app: App::new(kdf) }
    }

    pub fn call(&mut self, cmd: &str, args: &str) -> Result<String, JsValue> {
        let args: serde_json::Value = if args.is_empty() {
            serde_json::Value::Null
        } else {
            from_json(args)?
        };
        let out = self.app.call(cmd, &args).map_err(js_err)?;
        to_json(&out)
    }

    /// Returns `{ recovery_phrase }`.
    pub fn create(&mut self, passphrase: &str) -> Result<String, JsValue> {
        let c = self.app.create(passphrase).map_err(js_err)?;
        to_json(&c)
    }

    pub fn unlock(&mut self, file: &[u8], passphrase: &str) -> Result<(), JsValue> {
        self.app.unlock(file, passphrase).map_err(js_err)
    }

    pub fn recover(
        &mut self,
        file: &[u8],
        phrase: &str,
        new_passphrase: &str,
    ) -> Result<(), JsValue> {
        self.app
            .recover(file, phrase, new_passphrase)
            .map_err(js_err)
    }

    pub fn lock(&mut self) -> bool {
        self.app.lock()
    }

    pub fn is_unlocked(&self) -> bool {
        self.app.is_unlocked()
    }

    pub fn vault_id(&self) -> Option<String> {
        self.app.vault_id()
    }

    pub fn take_dirty(&mut self) -> bool {
        self.app.take_dirty()
    }

    pub fn seal(&self) -> Result<Vec<u8>, JsValue> {
        self.app.seal().map_err(js_err)
    }

    pub fn export_report(&self, request: &str) -> Result<ExportOut, JsValue> {
        let f = self
            .app
            .export_report(from_json(request)?)
            .map_err(js_err)?;
        Ok(ExportOut {
            meta: to_json(&f)?,
            bytes: f.bytes,
        })
    }

    pub fn inbox_import(&mut self, name: &str, bytes: &[u8]) -> Result<String, JsValue> {
        let r = self.app.inbox_import(name, bytes).map_err(js_err)?;
        to_json(&r)
    }
}
