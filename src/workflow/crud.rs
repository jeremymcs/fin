// Fin + Workflow Database CRUD

use crate::db::project::ProjectDb;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub blueprint_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub blueprint_id: String,
    pub section_id: String,
    pub title: String,
    pub one_liner: Option<String>,
    pub summary: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: i64,
    pub title: String,
    pub rationale: Option<String>,
    pub made_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationEvidence {
    pub task_id: String,
    pub command: String,
    pub exit_code: i32,
    pub verdict: String,
    pub duration_ms: i64,
}

pub struct WorkflowDb {
    db: ProjectDb,
}

impl WorkflowDb {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let db = ProjectDb::open(path)?;
        Ok(Self { db })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let db = ProjectDb::open_in_memory()?;
        Ok(Self { db })
    }

    // ── Blueprints ──────────────────────────────────────────────────────

    pub fn create_blueprint(&self, id: &str, title: &str, description: &str) -> anyhow::Result<()> {
        self.db.conn().execute(
            "INSERT INTO blueprints (id, title, description) VALUES (?1, ?2, ?3)",
            params![id, title, description],
        )?;
        Ok(())
    }

    pub fn get_blueprint(&self, id: &str) -> anyhow::Result<Option<Blueprint>> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, title, description, status FROM blueprints WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Blueprint {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_blueprint_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        let changed = self.db.conn().execute(
            "UPDATE blueprints SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        if changed == 0 {
            anyhow::bail!("blueprint not found: {}", id);
        }
        Ok(())
    }

    pub fn list_blueprints(&self) -> anyhow::Result<Vec<Blueprint>> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, title, description, status FROM blueprints ORDER BY created_at")?;
        let rows = stmt.query_map([], |row| {
            Ok(Blueprint {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── Sections ──────────────────────────────────────────────────────────

    pub fn create_section(&self, id: &str, blueprint_id: &str, title: &str) -> anyhow::Result<()> {
        self.db.conn().execute(
            "INSERT INTO sections (id, blueprint_id, title) VALUES (?1, ?2, ?3)",
            params![id, blueprint_id, title],
        )?;
        Ok(())
    }

    pub fn get_section(&self, id: &str) -> anyhow::Result<Option<Section>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, title, description, status FROM sections WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Section {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_section_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        let changed = self.db.conn().execute(
            "UPDATE sections SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        if changed == 0 {
            anyhow::bail!("section not found: {}", id);
        }
        Ok(())
    }

    pub fn list_sections(&self, blueprint_id: &str) -> anyhow::Result<Vec<Section>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, title, description, status FROM sections WHERE blueprint_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![blueprint_id], |row| {
            Ok(Section {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn next_pending_section(&self, blueprint_id: &str) -> anyhow::Result<Option<Section>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, title, description, status FROM sections
             WHERE blueprint_id = ?1 AND status = 'pending'
             ORDER BY created_at LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![blueprint_id], |row| {
            Ok(Section {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                status: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Tasks ───────────────────────────────────────────────────────────

    pub fn create_task(
        &self,
        id: &str,
        blueprint_id: &str,
        section_id: &str,
        title: &str,
    ) -> anyhow::Result<()> {
        self.db.conn().execute(
            "INSERT INTO tasks (id, blueprint_id, section_id, title) VALUES (?1, ?2, ?3, ?4)",
            params![id, blueprint_id, section_id, title],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> anyhow::Result<Option<Task>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, section_id, title, one_liner, summary, status FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Task {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                section_id: row.get(2)?,
                title: row.get(3)?,
                one_liner: row.get(4)?,
                summary: row.get(5)?,
                status: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_task_status(&self, id: &str, status: &str) -> anyhow::Result<()> {
        let changed = self.db.conn().execute(
            "UPDATE tasks SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        if changed == 0 {
            anyhow::bail!("task not found: {}", id);
        }
        Ok(())
    }

    pub fn update_task_summary(
        &self,
        id: &str,
        one_liner: &str,
        summary: &str,
    ) -> anyhow::Result<()> {
        let changed = self.db.conn().execute(
            "UPDATE tasks SET one_liner = ?1, summary = ?2, updated_at = datetime('now') WHERE id = ?3",
            params![one_liner, summary, id],
        )?;
        if changed == 0 {
            anyhow::bail!("task not found: {}", id);
        }
        Ok(())
    }

    pub fn list_tasks(&self, section_id: &str) -> anyhow::Result<Vec<Task>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, section_id, title, one_liner, summary, status FROM tasks WHERE section_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![section_id], |row| {
            Ok(Task {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                section_id: row.get(2)?,
                title: row.get(3)?,
                one_liner: row.get(4)?,
                summary: row.get(5)?,
                status: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn next_pending_task(&self, section_id: &str) -> anyhow::Result<Option<Task>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, blueprint_id, section_id, title, one_liner, summary, status FROM tasks
             WHERE section_id = ?1 AND status = 'pending'
             ORDER BY created_at LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![section_id], |row| {
            Ok(Task {
                id: row.get(0)?,
                blueprint_id: row.get(1)?,
                section_id: row.get(2)?,
                title: row.get(3)?,
                one_liner: row.get(4)?,
                summary: row.get(5)?,
                status: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // ── Decisions ───────────────────────────────────────────────────────

    pub fn add_decision(&self, title: &str, rationale: &str, made_by: &str) -> anyhow::Result<i64> {
        self.db.conn().execute(
            "INSERT INTO decisions (title, rationale, made_by) VALUES (?1, ?2, ?3)",
            params![title, rationale, made_by],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn list_decisions(&self) -> anyhow::Result<Vec<Decision>> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, title, rationale, made_by FROM decisions ORDER BY created_at")?;
        let rows = stmt.query_map([], |row| {
            Ok(Decision {
                id: row.get(0)?,
                title: row.get(1)?,
                rationale: row.get(2)?,
                made_by: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ── Validation Evidence ───────────────────────────────────────────

    pub fn add_evidence(
        &self,
        task_id: &str,
        command: &str,
        exit_code: i32,
        verdict: &str,
        duration_ms: i64,
    ) -> anyhow::Result<i64> {
        self.db.conn().execute(
            "INSERT INTO validation_evidence (task_id, command, exit_code, verdict, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task_id, command, exit_code, verdict, duration_ms],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    pub fn get_evidence(&self, task_id: &str) -> anyhow::Result<Vec<ValidationEvidence>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT task_id, command, exit_code, verdict, duration_ms FROM validation_evidence WHERE task_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(ValidationEvidence {
                task_id: row.get(0)?,
                command: row.get(1)?,
                exit_code: row.get(2)?,
                verdict: row.get(3)?,
                duration_ms: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> WorkflowDb {
        WorkflowDb::open_in_memory().expect("failed to open in-memory db")
    }

    #[test]
    fn blueprint_crud() {
        let db = setup();

        db.create_blueprint("B001", "First blueprint", "A description")
            .unwrap();
        let b = db.get_blueprint("B001").unwrap().expect("should exist");
        assert_eq!(b.title, "First blueprint");
        assert_eq!(b.status, "pending");

        db.update_blueprint_status("B001", "in_progress").unwrap();
        let b = db.get_blueprint("B001").unwrap().unwrap();
        assert_eq!(b.status, "in_progress");

        let all = db.list_blueprints().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn blueprint_not_found() {
        let db = setup();
        assert!(db.get_blueprint("NOPE").unwrap().is_none());
        assert!(db.update_blueprint_status("NOPE", "done").is_err());
    }

    #[test]
    fn section_crud() {
        let db = setup();
        db.create_blueprint("B001", "B", "d").unwrap();
        db.create_section("S01", "B001", "First section").unwrap();

        let s = db.get_section("S01").unwrap().expect("should exist");
        assert_eq!(s.blueprint_id, "B001");
        assert_eq!(s.status, "pending");

        db.update_section_status("S01", "done").unwrap();
        let s = db.get_section("S01").unwrap().unwrap();
        assert_eq!(s.status, "done");

        let all = db.list_sections("B001").unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn next_pending_section() {
        let db = setup();
        db.create_blueprint("B001", "B", "d").unwrap();
        db.create_section("S01", "B001", "First").unwrap();
        db.create_section("S02", "B001", "Second").unwrap();

        let next = db.next_pending_section("B001").unwrap().unwrap();
        assert_eq!(next.id, "S01");

        db.update_section_status("S01", "done").unwrap();
        let next = db.next_pending_section("B001").unwrap().unwrap();
        assert_eq!(next.id, "S02");

        db.update_section_status("S02", "done").unwrap();
        assert!(db.next_pending_section("B001").unwrap().is_none());
    }

    #[test]
    fn task_crud() {
        let db = setup();
        db.create_blueprint("B001", "B", "d").unwrap();
        db.create_section("S01", "B001", "Section").unwrap();
        db.create_task("T01", "B001", "S01", "First task").unwrap();

        let t = db.get_task("T01").unwrap().expect("should exist");
        assert_eq!(t.title, "First task");
        assert_eq!(t.status, "pending");
        assert!(t.one_liner.is_none());

        db.update_task_summary("T01", "short desc", "longer summary")
            .unwrap();
        let t = db.get_task("T01").unwrap().unwrap();
        assert_eq!(t.one_liner.as_deref(), Some("short desc"));
        assert_eq!(t.summary.as_deref(), Some("longer summary"));

        db.update_task_status("T01", "done").unwrap();
        let t = db.get_task("T01").unwrap().unwrap();
        assert_eq!(t.status, "done");
    }

    #[test]
    fn next_pending_task() {
        let db = setup();
        db.create_blueprint("B001", "B", "d").unwrap();
        db.create_section("S01", "B001", "Section").unwrap();
        db.create_task("T01", "B001", "S01", "First").unwrap();
        db.create_task("T02", "B001", "S01", "Second").unwrap();

        let next = db.next_pending_task("S01").unwrap().unwrap();
        assert_eq!(next.id, "T01");

        db.update_task_status("T01", "done").unwrap();
        let next = db.next_pending_task("S01").unwrap().unwrap();
        assert_eq!(next.id, "T02");
    }

    #[test]
    fn decisions() {
        let db = setup();
        let id = db
            .add_decision("Use Rust", "Performance matters", "team")
            .unwrap();
        assert!(id > 0);

        let all = db.list_decisions().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "Use Rust");
        assert_eq!(all[0].rationale.as_deref(), Some("Performance matters"));
    }

    #[test]
    fn validation_evidence() {
        let db = setup();
        db.create_blueprint("B001", "B", "d").unwrap();
        db.create_section("S01", "B001", "Section").unwrap();
        db.create_task("T01", "B001", "S01", "Task").unwrap();

        let id = db
            .add_evidence("T01", "cargo test", 0, "pass", 1234)
            .unwrap();
        assert!(id > 0);

        let ev = db.get_evidence("T01").unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].command, "cargo test");
        assert_eq!(ev[0].exit_code, 0);
        assert_eq!(ev[0].verdict, "pass");
        assert_eq!(ev[0].duration_ms, 1234);
    }
}
