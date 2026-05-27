#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProfile {
    Desktop,
    Pi,
}

impl RuntimeProfile {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Pi => "pi",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProfileSelection {
    pub(crate) profile: RuntimeProfile,
    pub(crate) requested: String,
    pub(crate) source: &'static str,
    pub(crate) pi_feature: bool,
    pub(crate) auto_pi_target: bool,
}

impl RuntimeProfileSelection {
    pub(crate) fn current() -> Self {
        let requested = std::env::var("DRAGONGUI_PROFILE").unwrap_or_else(|_| "auto".to_string());
        let normalized = requested.trim().to_ascii_lowercase();
        let pi_feature = cfg!(feature = "pi");
        let auto_pi_target = cfg!(all(target_os = "linux", target_arch = "aarch64"));
        let auto_profile = if pi_feature || auto_pi_target {
            RuntimeProfile::Pi
        } else {
            RuntimeProfile::Desktop
        };

        let (profile, source) = match normalized.as_str() {
            "" | "auto" => (auto_profile, "auto"),
            "desktop" => (RuntimeProfile::Desktop, "env"),
            "pi" | "rpi" | "raspberry-pi" | "raspberry_pi" => (RuntimeProfile::Pi, "env"),
            _ => (auto_profile, "invalid-env"),
        };

        Self {
            profile,
            requested,
            source,
            pi_feature,
            auto_pi_target,
        }
    }

    pub(crate) const fn use_pi_gpu_defaults(&self) -> bool {
        matches!(self.profile, RuntimeProfile::Pi)
    }

    pub(crate) const fn scatter_max_points(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(200_000),
        }
    }

    pub(crate) const fn scatter_lod_threshold(&self) -> Option<u32> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(50_000),
        }
    }

    pub(crate) const fn scatter_interactive_render_scale(&self) -> Option<f32> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(0.75),
        }
    }

    pub(crate) fn scatter_static_render_scale(&self) -> Option<f32> {
        std::env::var("DRAGONGUI_SCATTER_STATIC_RENDER_SCALE")
            .ok()
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| value.clamp(0.25, 1.0))
    }

    pub(crate) const fn line_plot_max_points(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(50_000),
        }
    }

    pub(crate) const fn line_plot_segment_budget(&self) -> usize {
        match self.profile {
            RuntimeProfile::Desktop => 4096,
            RuntimeProfile::Pi => 1536,
        }
    }

    pub(crate) const fn line_plot_simplify_styles(&self) -> bool {
        matches!(self.profile, RuntimeProfile::Pi)
    }

    pub(crate) const fn histogram_bin_budget(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(384),
        }
    }

    pub(crate) const fn histogram_compact_tick_count(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(4),
        }
    }

    pub(crate) const fn table_page_size(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(64),
        }
    }

    pub(crate) const fn table_sample_rows(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(512),
        }
    }

    pub(crate) const fn table_column_buffer_rows(&self) -> Option<usize> {
        match self.profile {
            RuntimeProfile::Desktop => None,
            RuntimeProfile::Pi => Some(10_000),
        }
    }

    pub(crate) const fn table_compact_metrics(&self) -> bool {
        matches!(self.profile, RuntimeProfile::Pi)
    }

    pub(crate) const fn pie_chart_compact_labels(&self) -> bool {
        matches!(self.profile, RuntimeProfile::Pi)
    }
}

pub(crate) const fn target_os() -> &'static str {
    std::env::consts::OS
}

pub(crate) const fn target_arch() -> &'static str {
    std::env::consts::ARCH
}

pub(crate) const fn embedded_webview_available() -> bool {
    cfg!(windows)
}

pub(crate) fn debug_log_enabled() -> bool {
    std::env::var("DRAGONGUI_LOG")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "debug" | "trace"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn env_can_force_pi_profile() {
        let _guard = lock_env();
        let previous = std::env::var("DRAGONGUI_PROFILE").ok();
        std::env::set_var("DRAGONGUI_PROFILE", "pi");

        let profile = RuntimeProfileSelection::current();

        assert_eq!(profile.profile, RuntimeProfile::Pi);
        assert_eq!(profile.source, "env");
        assert_eq!(profile.scatter_max_points(), Some(200_000));
        assert_eq!(profile.line_plot_max_points(), Some(50_000));
        assert_eq!(profile.line_plot_segment_budget(), 1536);
        assert!(profile.line_plot_simplify_styles());
        assert_eq!(profile.histogram_bin_budget(), Some(384));
        assert_eq!(profile.histogram_compact_tick_count(), Some(4));
        assert_eq!(profile.table_page_size(), Some(64));
        assert_eq!(profile.table_sample_rows(), Some(512));
        assert_eq!(profile.table_column_buffer_rows(), Some(10_000));
        assert!(profile.table_compact_metrics());
        assert!(profile.pie_chart_compact_labels());

        if let Some(previous) = previous {
            std::env::set_var("DRAGONGUI_PROFILE", previous);
        } else {
            std::env::remove_var("DRAGONGUI_PROFILE");
        }
    }

    #[test]
    fn env_can_force_desktop_profile() {
        let _guard = lock_env();
        let previous = std::env::var("DRAGONGUI_PROFILE").ok();
        std::env::set_var("DRAGONGUI_PROFILE", "desktop");

        let profile = RuntimeProfileSelection::current();

        assert_eq!(profile.profile, RuntimeProfile::Desktop);
        assert_eq!(profile.source, "env");
        assert_eq!(profile.scatter_max_points(), None);
        assert_eq!(profile.line_plot_max_points(), None);
        assert_eq!(profile.line_plot_segment_budget(), 4096);
        assert!(!profile.line_plot_simplify_styles());
        assert_eq!(profile.histogram_bin_budget(), None);
        assert_eq!(profile.histogram_compact_tick_count(), None);
        assert_eq!(profile.table_page_size(), None);
        assert_eq!(profile.table_sample_rows(), None);
        assert_eq!(profile.table_column_buffer_rows(), None);
        assert!(!profile.table_compact_metrics());
        assert!(!profile.pie_chart_compact_labels());

        if let Some(previous) = previous {
            std::env::set_var("DRAGONGUI_PROFILE", previous);
        } else {
            std::env::remove_var("DRAGONGUI_PROFILE");
        }
    }

    #[test]
    fn invalid_env_falls_back_to_auto_selection() {
        let _guard = lock_env();
        let previous = std::env::var("DRAGONGUI_PROFILE").ok();
        std::env::set_var("DRAGONGUI_PROFILE", "unknown-profile");

        let profile = RuntimeProfileSelection::current();

        assert_eq!(profile.source, "invalid-env");
        let expected =
            if cfg!(feature = "pi") || cfg!(all(target_os = "linux", target_arch = "aarch64")) {
                RuntimeProfile::Pi
            } else {
                RuntimeProfile::Desktop
            };
        assert_eq!(profile.profile, expected);

        if let Some(previous) = previous {
            std::env::set_var("DRAGONGUI_PROFILE", previous);
        } else {
            std::env::remove_var("DRAGONGUI_PROFILE");
        }
    }

    #[test]
    fn debug_log_env_accepts_debug_values() {
        let _guard = lock_env();
        let previous = std::env::var("DRAGONGUI_LOG").ok();
        std::env::set_var("DRAGONGUI_LOG", "debug");

        assert!(debug_log_enabled());

        if let Some(previous) = previous {
            std::env::set_var("DRAGONGUI_LOG", previous);
        } else {
            std::env::remove_var("DRAGONGUI_LOG");
        }
    }

    #[test]
    fn debug_log_env_ignores_other_values() {
        let _guard = lock_env();
        let previous = std::env::var("DRAGONGUI_LOG").ok();
        std::env::set_var("DRAGONGUI_LOG", "info");

        assert!(!debug_log_enabled());

        if let Some(previous) = previous {
            std::env::set_var("DRAGONGUI_LOG", previous);
        } else {
            std::env::remove_var("DRAGONGUI_LOG");
        }
    }
}
