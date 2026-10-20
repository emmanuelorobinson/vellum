//! Native-person share actions are outside the model tool registry and do not mutate DocStore.
use super::*;
use crate::daemon::share::{self, Uploads};
impl McpServer {
    pub fn with_shares(mut self, state: PathBuf) -> Self {
        self.shares = Some(Uploads::new(state.join("shares")));
        self
    }
    pub(super) fn share_action(&mut self, conn: u64, p: &Value) -> Result<Value, (i64, String)> {
        let error = |message: &str| (-32000, message.to_owned());
        if self.client(conn).is_none() {
            return Err(error("Initialize the host connection first"));
        }
        let action = p["action"]
            .as_str()
            .ok_or_else(|| error("Missing share action"))?;
        let file = p["document"]
            .as_str()
            .ok_or_else(|| error("Missing document"))?;
        self.session(conn);
        let path = ToolCtx::new(
            &mut self.docs,
            self.sessions.get_mut(&conn).expect("session"),
        )
        .resolve(file)
        .map_err(|_| error("Document unavailable"))?;
        if action == "status" {
            return Ok(self
                .shares
                .as_mut()
                .ok_or_else(|| error("Sharing unavailable"))?
                .status(&path));
        }
        if action == "retry" {
            return self
                .shares
                .as_mut()
                .ok_or_else(|| error("Sharing unavailable"))?
                .start(&path, None, None, None)
                .map_err(|e| error(&e));
        }
        if action == "delete" {
            let id = p["id"].as_str().ok_or_else(|| error("Missing share id"))?;
            return self
                .shares
                .as_mut()
                .ok_or_else(|| error("Sharing unavailable"))?
                .revoke(&path, id)
                .map_err(|e| error(&e));
        }
        if !["check", "create"].contains(&action) {
            return Err(error("Invalid share action"));
        }
        let doc = self
            .docs
            .target(&path)
            .and_then(|t| t.snapshot())
            .map_err(|_| error("Document unavailable"))?;
        let (fs, entry) = crate::daemon::production::share_files(&path)
            .map_err(|_| error("Snapshot files unavailable"))?;
        let bundle = share::collect(fs, &entry, &doc).map_err(|e| error(&e))?;
        let source_hash = vellum_assets_fs::content_hash(&bundle.bytes().map_err(|e| error(&e))?);
        if action == "check" {
            let mut status = self
                .shares
                .as_mut()
                .ok_or_else(|| error("Sharing unavailable"))?
                .status(&path);
            if status["state"] == "shared" {
                if status["sourceHash"] != source_hash {
                    status["state"] = json!("changed");
                }
            } else if status["state"] != "uploading" && status["state"] != "failed" {
                status = json!({"state":if bundle.missing.is_empty(){"not-shared"}else{"checking"},"missing":bundle.missing});
            }
            return Ok(status);
        }
        if !bundle.missing.is_empty() && p["shareAnyway"] != true {
            let mut state = json!({"state":"checking","missing":bundle.missing});
            if let Some(id) = p["id"].as_str() {
                state["id"] = json!(id);
            }
            return Ok(state);
        }
        let id = p["id"].as_str().map(str::to_owned);
        self.shares
            .as_mut()
            .ok_or_else(|| error("Sharing unavailable"))?
            .start(&path, Some(bundle), id, Some(source_hash))
            .map_err(|e| error(&e))
    }
}
