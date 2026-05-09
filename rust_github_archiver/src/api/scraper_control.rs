use crate::api::state::AppState;
use crate::audit::AuditAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScraperControlAction {
    Start,
    Stop,
    Pause,
    Resume,
    Restart,
}

#[derive(Debug, Clone)]
pub struct ScraperControlResult {
    pub status: &'static str,
    pub message: &'static str,
    pub scraper_running: bool,
}

impl ScraperControlAction {
    pub fn from_api_action(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "pause" => Some(Self::Pause),
            "resume" => Some(Self::Resume),
            "restart" => Some(Self::Restart),
            _ => None,
        }
    }

    pub fn audit_action(self) -> AuditAction {
        match self {
            Self::Start => AuditAction::ScraperStarted,
            Self::Stop => AuditAction::ScraperStopped,
            Self::Pause => AuditAction::ScraperPaused,
            Self::Resume => AuditAction::ScraperResumed,
            Self::Restart => AuditAction::ScraperRestarted,
        }
    }

    pub fn failure_prefix(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Restart => "restart",
        }
    }

    fn success_status(self) -> &'static str {
        match self {
            Self::Start => "success",
            Self::Stop => "success",
            Self::Pause => "success",
            Self::Resume => "success",
            Self::Restart => "success",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            Self::Start => "Scraper started successfully",
            Self::Stop => "Scraper stopped successfully",
            Self::Pause => "Scraper paused successfully",
            Self::Resume => "Scraper resumed successfully",
            Self::Restart => "Scraper restarted successfully",
        }
    }
}

pub async fn execute_scraper_action(
    app_state: &AppState,
    action: ScraperControlAction,
) -> Result<ScraperControlResult, String> {
    let runtime_result = match action {
        ScraperControlAction::Start => app_state.scraper_runtime.start().await,
        ScraperControlAction::Stop => app_state.scraper_runtime.stop().await,
        ScraperControlAction::Pause => app_state.scraper_runtime.pause().await,
        ScraperControlAction::Resume => app_state.scraper_runtime.resume().await,
        ScraperControlAction::Restart => app_state.scraper_runtime.restart().await,
    };

    runtime_result.map_err(|error| error.to_string())?;

    Ok(ScraperControlResult {
        status: action.success_status(),
        message: action.success_message(),
        scraper_running: app_state.scraper_manager.is_running(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_metadata_matches_audit_events() {
        assert_eq!(
            ScraperControlAction::Start.audit_action(),
            AuditAction::ScraperStarted
        );
        assert_eq!(
            ScraperControlAction::Restart.audit_action(),
            AuditAction::ScraperRestarted
        );
        assert_eq!(ScraperControlAction::Pause.failure_prefix(), "pause");
        assert_eq!(
            ScraperControlAction::from_api_action("resume"),
            Some(ScraperControlAction::Resume)
        );
    }
}
