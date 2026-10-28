//! Archivio SQLite: conversazioni, esecuzioni e passi. Le migrazioni usano `user_version`.

use crate::agent::messaggi::Messaggio;
use crate::agent::{Passo, RunView};
use crate::error::{AppError, Result};
use crate::providers::Usage;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub struct Db(Mutex<Connection>);

const MIGRAZIONI: &[&str] = &[
    // 1: struttura iniziale
    "CREATE TABLE conversazioni (
        id TEXT PRIMARY KEY,
        agent_id TEXT NOT NULL,
        titolo TEXT NOT NULL,
        creato INTEGER NOT NULL,
        aggiornato INTEGER NOT NULL,
        trascrizione TEXT NOT NULL DEFAULT '[]'
     );
     CREATE INDEX conversazioni_agente ON conversazioni(agent_id, aggiornato DESC);
     CREATE TABLE esecuzioni (
        id TEXT PRIMARY KEY,
        conversazione_id TEXT NOT NULL,
        agent_id TEXT NOT NULL,
        titolo TEXT NOT NULL,
        sorgente TEXT NOT NULL,
        stato TEXT NOT NULL,
        inizio INTEGER NOT NULL,
        fine INTEGER,
        errore TEXT,
        usage TEXT,
        costo REAL,
        modello_servito TEXT
     );
     CREATE INDEX esecuzioni_conversazione ON esecuzioni(conversazione_id, inizio);
     CREATE INDEX esecuzioni_inizio ON esecuzioni(inizio DESC);
     CREATE TABLE passi (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL,
        indice INTEGER NOT NULL,
        dati TEXT NOT NULL
     );
     CREATE INDEX passi_run ON passi(run_id, indice);",
    // 2: registro delle azioni degli agenti
    "CREATE TABLE audit (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        ts INTEGER NOT NULL,
        run_id TEXT NOT NULL,
        agent_id TEXT NOT NULL,
        strumento TEXT NOT NULL,
        input TEXT NOT NULL,
        decisione TEXT NOT NULL,
        da TEXT NOT NULL,
        ok INTEGER NOT NULL,
        errore TEXT
     );
     CREATE INDEX audit_ts ON audit(ts DESC);",
    // 3: notifiche raccolte (sistema, Teams, Slack, posta) e stato delle fonti
    "CREATE TABLE notifiche (
        id TEXT PRIMARY KEY,
        ts INTEGER NOT NULL,
        app TEXT NOT NULL,
        app_id TEXT NOT NULL,
        titolo TEXT NOT NULL,
        testo TEXT NOT NULL,
        origine TEXT NOT NULL,
        letta INTEGER NOT NULL DEFAULT 0
     );
     CREATE INDEX notifiche_ts ON notifiche(ts DESC);
     CREATE TABLE stato_fonti (chiave TEXT PRIMARY KEY, valore TEXT NOT NULL);",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversazione {
    pub id: String,
    pub agent_id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let versione: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRAZIONI.iter().enumerate().skip(versione.max(0) as usize) {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        // Le esecuzioni rimaste aperte da un'uscita improvvisa non ripartono
        conn.execute(
            "UPDATE esecuzioni SET stato = 'cancelled', fine = inizio WHERE stato IN ('running', 'awaitingApproval')",
            [],
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---------- Conversazioni ----------

    pub fn crea_conversazione(&self, id: &str, agent_id: &str, titolo: &str, t: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO conversazioni (id, agent_id, titolo, creato, aggiornato) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, agent_id, titolo, t],
        )?;
        Ok(())
    }

    pub fn trascrizione(&self, id: &str) -> Result<(String, Vec<Messaggio>)> {
        let row: Option<(String, String)> = self
            .conn()
            .query_row("SELECT agent_id, trascrizione FROM conversazioni WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let (agent, json) = row.ok_or_else(|| AppError::msg("Conversazione non trovata"))?;
        Ok((agent, serde_json::from_str(&json)?))
    }

    pub fn salva_trascrizione(&self, id: &str, messaggi: &[Messaggio], t: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE conversazioni SET trascrizione = ?2, aggiornato = ?3 WHERE id = ?1",
            params![id, serde_json::to_string(messaggi)?, t],
        )?;
        Ok(())
    }

    pub fn conversazioni(&self, agent_id: &str, limite: u32) -> Result<Vec<Conversazione>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, agent_id, titolo, creato, aggiornato FROM conversazioni
             WHERE agent_id = ?1 ORDER BY aggiornato DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![agent_id, limite], |r| {
            Ok(Conversazione { id: r.get(0)?, agent_id: r.get(1)?, title: r.get(2)?, created_at: r.get(3)?, updated_at: r.get(4)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn elimina_conversazione(&self, id: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM passi WHERE run_id IN (SELECT id FROM esecuzioni WHERE conversazione_id = ?1)",
            [id],
        )?;
        conn.execute("DELETE FROM esecuzioni WHERE conversazione_id = ?1", [id])?;
        conn.execute("DELETE FROM conversazioni WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn elimina_conversazioni_agente(&self, agent_id: &str) -> Result<Vec<String>> {
        let ids: Vec<String> = {
            let conn = self.conn();
            let mut st = conn.prepare("SELECT id FROM conversazioni WHERE agent_id = ?1")?;
            let rows = st.query_map([agent_id], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for id in &ids {
            self.elimina_conversazione(id)?;
        }
        Ok(ids)
    }

    // ---------- Registro delle azioni ----------

    #[allow(clippy::too_many_arguments)]
    pub fn audit(&self, run_id: &str, agent_id: &str, strumento: &str, input: &serde_json::Value, decisione: &str, da: &str, ok: bool, errore: Option<&str>) -> Result<()> {
        self.conn().execute(
            "INSERT INTO audit (ts, run_id, agent_id, strumento, input, decisione, da, ok, errore) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![crate::store::now_ms(), run_id, agent_id, strumento, input.to_string(), decisione, da, ok, errore],
        )?;
        Ok(())
    }

    pub fn audit_lista(&self, limite: u32, offset: u32) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT ts, run_id, agent_id, strumento, input, decisione, da, ok, errore FROM audit ORDER BY ts DESC LIMIT ?1 OFFSET ?2")?;
        let rows = st.query_map(params![limite, offset], |r| {
            Ok(serde_json::json!({
                "at": r.get::<_, i64>(0)?, "runId": r.get::<_, String>(1)?, "agentId": r.get::<_, String>(2)?, "tool": r.get::<_, String>(3)?,
                "input": serde_json::from_str::<serde_json::Value>(&r.get::<_, String>(4)?).unwrap_or_default(),
                "decision": r.get::<_, String>(5)?, "by": r.get::<_, String>(6)?, "ok": r.get::<_, bool>(7)?, "error": r.get::<_, Option<String>>(8)?,
            }))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn pulisci(&self, prima_di: i64) -> Result<usize> {
        let conn = self.conn();
        conn.execute("DELETE FROM passi WHERE run_id IN (SELECT id FROM esecuzioni WHERE inizio < ?1)", [prima_di])?;
        let n = conn.execute("DELETE FROM esecuzioni WHERE inizio < ?1", [prima_di])?;
        conn.execute("DELETE FROM audit WHERE ts < ?1", [prima_di])?;
        conn.execute("DELETE FROM conversazioni WHERE aggiornato < ?1", [prima_di])?;
        Ok(n)
    }

    // ---------- Notifiche ----------

    /// Salva una notifica; `false` se c'era già.
    pub fn notifica(&self, n: &crate::notifiche::Notifica) -> Result<bool> {
        let k = self.conn().execute(
            "INSERT OR IGNORE INTO notifiche (id, ts, app, app_id, titolo, testo, origine) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![n.id, n.at, n.app, n.app_id, n.title, n.body, n.source],
        )?;
        Ok(k > 0)
    }

    pub fn notifiche(&self, da: i64, app: Option<&str>, testo: Option<&str>, limite: u32) -> Result<Vec<crate::notifiche::Notifica>> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT id, ts, app, app_id, titolo, testo, origine, letta FROM notifiche
             WHERE ts >= ?1 AND (?2 IS NULL OR lower(app) LIKE '%' || lower(?2) || '%')
             AND (?3 IS NULL OR lower(titolo || ' ' || testo) LIKE '%' || lower(?3) || '%')
             ORDER BY ts DESC LIMIT ?4",
        )?;
        let rows = st.query_map(params![da, app, testo, limite], |r| {
            Ok(crate::notifiche::Notifica {
                id: r.get(0)?,
                at: r.get(1)?,
                app: r.get(2)?,
                app_id: r.get(3)?,
                title: r.get(4)?,
                body: r.get(5)?,
                source: r.get(6)?,
                read: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn notifiche_lette(&self, prima_di: i64) -> Result<usize> {
        Ok(self.conn().execute("UPDATE notifiche SET letta = 1 WHERE ts <= ?1", [prima_di])?)
    }

    pub fn stato_fonte(&self, k: &str) -> Option<String> {
        self.conn().query_row("SELECT valore FROM stato_fonti WHERE chiave = ?1", [k], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn salva_stato_fonte(&self, k: &str, v: &str) {
        let _ = self.conn().execute("INSERT INTO stato_fonti (chiave, valore) VALUES (?1, ?2) ON CONFLICT(chiave) DO UPDATE SET valore = excluded.valore", params![k, v]);
    }

    // ---------- Esecuzioni ----------

    pub fn crea_run(&self, r: &RunView) -> Result<()> {
        self.conn().execute(
            "INSERT INTO esecuzioni (id, conversazione_id, agent_id, titolo, sorgente, stato, inizio)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![r.id, r.conversation_id, r.agent_id, r.title, r.trigger, r.status, r.started_at],
        )?;
        Ok(())
    }

    pub fn chiudi_run(
        &self,
        id: &str,
        stato: &str,
        fine: i64,
        errore: Option<&str>,
        usage: &Usage,
        costo: Option<f64>,
        modello: Option<&str>,
    ) -> Result<()> {
        self.conn().execute(
            "UPDATE esecuzioni SET stato = ?2, fine = ?3, errore = ?4, usage = ?5, costo = ?6, modello_servito = ?7 WHERE id = ?1",
            params![id, stato, fine, errore, serde_json::to_string(usage)?, costo, modello],
        )?;
        Ok(())
    }

    pub fn salva_passo(&self, run_id: &str, indice: usize, p: &Passo) -> Result<()> {
        self.conn().execute(
            "INSERT INTO passi (id, run_id, indice, dati) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET dati = excluded.dati",
            params![p.id, run_id, indice as i64, serde_json::to_string(p)?],
        )?;
        Ok(())
    }

    fn leggi_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<RunView> {
        let usage: Option<String> = r.get(9)?;
        Ok(RunView {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            agent_id: r.get(2)?,
            title: r.get(3)?,
            trigger: r.get(4)?,
            status: r.get(5)?,
            started_at: r.get(6)?,
            ended_at: r.get(7)?,
            error: r.get(8)?,
            usage: usage.and_then(|u| serde_json::from_str(&u).ok()),
            cost_estimate: r.get(10)?,
            served_model: r.get(11)?,
            steps: vec![],
        })
    }

    const COLONNE: &'static str =
        "id, conversazione_id, agent_id, titolo, sorgente, stato, inizio, fine, errore, usage, costo, modello_servito";

    fn passi(&self, run_id: &str) -> Result<Vec<Passo>> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT dati FROM passi WHERE run_id = ?1 ORDER BY indice")?;
        let rows = st.query_map([run_id], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|j| j.ok().and_then(|j| serde_json::from_str(&j).ok())).collect())
    }

    pub fn run(&self, id: &str) -> Result<RunView> {
        let mut run = self
            .conn()
            .query_row(&format!("SELECT {} FROM esecuzioni WHERE id = ?1", Self::COLONNE), [id], Self::leggi_run)
            .optional()?
            .ok_or_else(|| AppError::msg("Esecuzione non trovata"))?;
        run.steps = self.passi(id)?;
        Ok(run)
    }

    /// Tutte le esecuzioni di una conversazione, con i passi: è la chat da ridisegnare.
    pub fn runs_conversazione(&self, conversazione_id: &str) -> Result<Vec<RunView>> {
        let mut runs: Vec<RunView> = {
            let conn = self.conn();
            let mut st = conn.prepare(&format!(
                "SELECT {} FROM esecuzioni WHERE conversazione_id = ?1 ORDER BY inizio",
                Self::COLONNE
            ))?;
            let rows = st.query_map([conversazione_id], Self::leggi_run)?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for r in &mut runs {
            r.steps = self.passi(&r.id)?;
        }
        Ok(runs)
    }

    pub fn runs(&self, agent_id: Option<&str>, limite: u32, offset: u32) -> Result<Vec<RunView>> {
        let conn = self.conn();
        let mut st = conn.prepare(&format!(
            "SELECT {} FROM esecuzioni WHERE (?1 IS NULL OR agent_id = ?1) ORDER BY inizio DESC LIMIT ?2 OFFSET ?3",
            Self::COLONNE
        ))?;
        let rows = st.query_map(params![agent_id, limite, offset], Self::leggi_run)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}
