//! Paired devices: each has a name, a role and the public half of its own key. Revoking a
//! device deletes its key, so it can't sign in again.
use super::{Store, new_id, now_ms};
use anyhow::{Result, bail};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// `owner`, `approve` or `view`.
    pub role: String,
    /// The device's P-256 public key: the uncompressed point, base64.
    #[serde(skip)]
    pub public_key: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Device> {
    Ok(Device { id: r.get(0)?, name: r.get(1)?, role: r.get(2)?, public_key: r.get(3)?, created_at: r.get(4)?, last_seen_at: r.get(5)? })
}

const COLUMNS: &str = "id, name, role, public_key, created_at, last_seen_at";

impl Store {
    pub fn device_add(&self, name: &str, role: &str, public_key: &str) -> Result<Device> {
        if !["owner", "approve", "view"].contains(&role) {
            bail!("unknown role {role:?}");
        }
        let id = new_id();
        self.conn().execute(
            "INSERT INTO devices (id, name, role, public_key, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, role, public_key, now_ms()],
        )?;
        self.device(&id)?.ok_or_else(|| anyhow::anyhow!("the device vanished as it was added"))
    }

    pub fn device(&self, id: &str) -> Result<Option<Device>> {
        let sql = format!("SELECT {COLUMNS} FROM devices WHERE id = ?1");
        Ok(self.conn().query_row(&sql, [id], row).optional()?)
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let sql = format!("SELECT {COLUMNS} FROM devices ORDER BY created_at");
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Deletes the device's key. False if there was no such device.
    pub fn device_revoke(&self, id: &str) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM devices WHERE id = ?1", [id])? > 0)
    }

    pub fn device_seen(&self, id: &str) -> Result<()> {
        self.conn().execute("UPDATE devices SET last_seen_at = ?2 WHERE id = ?1", params![id, now_ms()])?;
        Ok(())
    }

    /// Records who answered a request: a device id, or `local`.
    pub fn request_answered_by(&self, thread_id: &str, request_id: &str, by: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE requests SET answered_by = ?3 WHERE thread_id = ?1 AND request_id = ?2",
            params![thread_id, request_id, by],
        )?;
        Ok(())
    }

    /// Where a device wants its notifications (one endpoint per browser install).
    pub fn push_subscribe(&self, device_id: &str, endpoint: &str, p256dh: &str, auth: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO push_subscriptions (endpoint, device_id, p256dh, auth, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(endpoint) DO UPDATE SET device_id = ?2, p256dh = ?3, auth = ?4",
            params![endpoint, device_id, p256dh, auth, now_ms()],
        )?;
        Ok(())
    }

    pub fn push_unsubscribe(&self, endpoint: &str) -> Result<()> {
        self.conn().execute("DELETE FROM push_subscriptions WHERE endpoint = ?1", [endpoint])?;
        Ok(())
    }

    /// (endpoint, p256dh, auth) of every device that may answer requests.
    pub fn push_targets(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT s.endpoint, s.p256dh, s.auth FROM push_subscriptions s JOIN devices d ON d.id = s.device_id
             WHERE d.role IN ('owner', 'approve')",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
