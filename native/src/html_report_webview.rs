use serde_json::Value;
use winit::window::Window;

use crate::{document::WidgetNode, layout::LayoutResult};

pub(crate) struct HtmlReportWebViewManager {
    inner: platform::PlatformHtmlReportWebViewManager,
}

impl HtmlReportWebViewManager {
    pub(crate) fn new(window: &Window) -> Self {
        Self {
            inner: platform::PlatformHtmlReportWebViewManager::new(window),
        }
    }

    pub(crate) fn sync(&mut self, tree: Option<&WidgetNode>, layout: Option<&LayoutResult>) {
        self.inner.sync(tree, layout);
    }

    pub(crate) fn hide_all(&mut self) {
        self.inner.hide_all();
    }

    pub(crate) fn snapshot(&self) -> Value {
        self.inner.snapshot()
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        collections::{HashMap, HashSet},
        ffi::c_void,
        fs,
        path::{Path, PathBuf},
        process,
        sync::mpsc,
    };

    use serde_json::{json, Value};
    use webview2_com::{
        CoTaskMemPWSTR, CoreWebView2EnvironmentOptions,
        CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
            ICoreWebView2Environment, ICoreWebView2EnvironmentOptions,
        },
    };
    use windows::{
        core::Error as WindowsError,
        Win32::{
            Foundation::{E_POINTER, HWND, RECT},
            System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED},
        },
    };
    use winit::{
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
        window::Window,
    };

    use crate::{
        document::{WidgetKind, WidgetNode},
        layout::{LayoutResult, Rect},
    };

    pub(crate) struct PlatformHtmlReportWebViewManager {
        hwnd: Option<HWND>,
        environment: Option<ICoreWebView2Environment>,
        views: HashMap<String, HtmlReportView>,
        enabled: bool,
        reason: Option<String>,
        last_error: Option<String>,
        user_data_dir: Option<PathBuf>,
        profile_generation: u32,
        profile_recovered: bool,
    }

    struct HtmlReportView {
        controller: ICoreWebView2Controller,
        webview: ICoreWebView2,
        source: Option<ReportSource>,
        rect: Option<[i32; 4]>,
        visible: bool,
        allow_scripts: Option<bool>,
        status: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ReportSource {
        Url(String),
        Html(String),
        Blocked(String),
        Empty,
    }

    impl PlatformHtmlReportWebViewManager {
        pub(crate) fn new(window: &Window) -> Self {
            let forced = std::env::var("DRAGONGUI_HTMLREPORT_WEBVIEW2").ok();
            let smoke = std::env::var("DRAGONGUI_SMOKE_FRAMES").is_ok();
            if forced.as_deref() == Some("0") || (smoke && forced.as_deref() != Some("1")) {
                return Self {
                    hwnd: None,
                    environment: None,
                    views: HashMap::new(),
                    enabled: false,
                    reason: Some(if smoke {
                        "disabled during smoke runs; set DRAGONGUI_HTMLREPORT_WEBVIEW2=1 to force"
                            .to_string()
                    } else {
                        "disabled by DRAGONGUI_HTMLREPORT_WEBVIEW2=0".to_string()
                    }),
                    last_error: None,
                    user_data_dir: None,
                    profile_generation: 0,
                    profile_recovered: false,
                };
            }

            let hwnd = hwnd_from_window(window);
            Self {
                hwnd,
                environment: None,
                views: HashMap::new(),
                enabled: hwnd.is_some(),
                reason: hwnd
                    .is_none()
                    .then(|| "window does not expose a Win32 HWND".to_string()),
                last_error: None,
                user_data_dir: None,
                profile_generation: 0,
                profile_recovered: false,
            }
        }

        pub(crate) fn sync(&mut self, tree: Option<&WidgetNode>, layout: Option<&LayoutResult>) {
            if !self.enabled {
                return;
            }
            let Some(tree) = tree else {
                self.hide_all();
                return;
            };
            let Some(layout) = layout else {
                self.hide_all();
                return;
            };

            let mut reports = Vec::new();
            collect_visible_reports(tree, layout, &mut reports);
            if reports.is_empty() {
                self.hide_all();
                return;
            }

            let mut active = HashSet::new();
            for (id, source, rect, allow_scripts) in reports.into_iter().take(1) {
                active.insert(id.clone());
                if let Err(error) = self.sync_one(&id, source, rect, allow_scripts) {
                    if is_webview_initialization_error(&error) {
                        self.enabled = false;
                        self.reason = Some(
                            "WebView2 initialization failed; using native fallback".to_string(),
                        );
                    }
                    self.last_error = Some(error);
                    self.hide_view(&id);
                }
            }

            let stale = self
                .views
                .keys()
                .filter(|id| !active.contains(*id))
                .cloned()
                .collect::<Vec<_>>();
            for id in stale {
                self.hide_view(&id);
            }
        }

        pub(crate) fn hide_all(&mut self) {
            let ids = self.views.keys().cloned().collect::<Vec<_>>();
            for id in ids {
                self.hide_view(&id);
            }
        }

        pub(crate) fn snapshot(&self) -> Value {
            json!({
                "platform": "windows",
                "enabled": self.enabled,
                "reason": self.reason.as_deref(),
                "last_error": self.last_error.as_deref(),
                "user_data_dir": self.user_data_dir.as_ref().map(|path| path.display().to_string()),
                "profile_generation": self.profile_generation,
                "profile_recovered": self.profile_recovered,
                "environment_ready": self.environment.is_some(),
                "instances": self.views.iter().map(|(id, view)| {
                    (id.clone(), json!({
                        "visible": view.visible,
                        "rect": view.rect,
                        "status": view.status,
                        "source": source_label(view.source.as_ref()),
                        "allow_scripts": view.allow_scripts,
                    }))
                }).collect::<serde_json::Map<_, _>>(),
            })
        }

        fn sync_one(
            &mut self,
            id: &str,
            source: ReportSource,
            rect: [i32; 4],
            allow_scripts: bool,
        ) -> Result<(), String> {
            if matches!(source, ReportSource::Blocked(_) | ReportSource::Empty) {
                self.hide_view(id);
                let view = self.views.get_mut(id);
                if let Some(view) = view {
                    view.source = Some(source.clone());
                    view.status = status_for_source(&source);
                }
                return Ok(());
            }

            self.ensure_environment()?;
            if !self.views.contains_key(id) {
                let view = self.create_view_with_profile_recovery()?;
                self.views.insert(id.to_string(), view);
            }

            let Some(view) = self.views.get_mut(id) else {
                return Err("failed to retain WebView2 controller".to_string());
            };
            view.set_bounds(rect)?;
            view.set_scripts_enabled(allow_scripts)?;
            view.show(true)?;
            if view.source.as_ref() != Some(&source) {
                view.navigate(&source)?;
                view.source = Some(source.clone());
            }
            view.status = status_for_source(&source);
            Ok(())
        }

        fn ensure_environment(&mut self) -> Result<(), String> {
            if self.environment.is_some() {
                return Ok(());
            }
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                    .ok()
                    .map_err(|error| format!("COM init failed: {error}"))?;
            }

            let user_data_dir = webview_user_data_dir(self.profile_generation)?;
            let user_data_dir_text = user_data_dir.display().to_string();
            let browser_folder = CoTaskMemPWSTR::from("");
            let user_data_folder = CoTaskMemPWSTR::from(user_data_dir_text.as_str());
            let options: ICoreWebView2EnvironmentOptions =
                CoreWebView2EnvironmentOptions::default().into();
            self.user_data_dir = Some(user_data_dir);

            let (tx, rx) = mpsc::channel();
            CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    CreateCoreWebView2EnvironmentWithOptions(
                        *browser_folder.as_ref().as_pcwstr(),
                        *user_data_folder.as_ref().as_pcwstr(),
                        &options,
                        &handler,
                    )
                    .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, environment| {
                    error_code?;
                    tx.send(environment.ok_or_else(|| WindowsError::from(E_POINTER)))
                        .map_err(|_| WindowsError::from(E_POINTER))?;
                    Ok(())
                }),
            )
            .map_err(|error| format!("WebView2 environment failed: {error}"))?;

            self.environment = Some(
                rx.recv()
                    .map_err(|_| "WebView2 environment callback failed".to_string())?
                    .map_err(|error| format!("WebView2 environment failed: {error}"))?,
            );
            Ok(())
        }

        fn create_view_with_profile_recovery(&mut self) -> Result<HtmlReportView, String> {
            match self.create_view() {
                Ok(view) => Ok(view),
                Err(first_error)
                    if should_retry_with_fresh_profile(&first_error)
                        && self.profile_generation == 0 =>
                {
                    self.environment = None;
                    self.profile_generation = 1;
                    self.profile_recovered = true;
                    self.ensure_environment()?;
                    self.create_view().map_err(|second_error| {
                        format!("{second_error}; first attempt failed with: {first_error}")
                    })
                }
                Err(error) => Err(error),
            }
        }

        fn create_view(&self) -> Result<HtmlReportView, String> {
            let Some(parent) = self.hwnd else {
                return Err("missing Win32 parent HWND".to_string());
            };
            let Some(environment) = self.environment.as_ref() else {
                return Err("missing WebView2 environment".to_string());
            };

            let (tx, rx) = mpsc::channel();
            let environment = environment.clone();
            CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    environment
                        .CreateCoreWebView2Controller(parent, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, controller| {
                    error_code?;
                    tx.send(controller.ok_or_else(|| WindowsError::from(E_POINTER)))
                        .map_err(|_| WindowsError::from(E_POINTER))?;
                    Ok(())
                }),
            )
            .map_err(|error| format!("WebView2 controller failed: {error}"))?;

            let controller = rx
                .recv()
                .map_err(|_| "WebView2 controller callback failed".to_string())?
                .map_err(|error| format!("WebView2 controller failed: {error}"))?;
            unsafe {
                controller
                    .SetIsVisible(false)
                    .map_err(|error| format!("WebView2 visibility failed: {error}"))?;
            }
            let webview = unsafe {
                controller
                    .CoreWebView2()
                    .map_err(|error| format!("WebView2 core failed: {error}"))?
            };
            unsafe {
                if let Ok(settings) = webview.Settings() {
                    let _ = settings.SetAreDefaultContextMenusEnabled(true);
                    let _ = settings.SetAreDevToolsEnabled(true);
                }
            }
            Ok(HtmlReportView {
                controller,
                webview,
                source: None,
                rect: None,
                visible: false,
                allow_scripts: None,
                status: "created".to_string(),
            })
        }

        fn hide_view(&mut self, id: &str) {
            if let Some(view) = self.views.get_mut(id) {
                let _ = view.show(false);
            }
        }
    }

    impl HtmlReportView {
        fn set_bounds(&mut self, rect: [i32; 4]) -> Result<(), String> {
            if self.rect == Some(rect) {
                return Ok(());
            }
            let bounds = RECT {
                left: rect[0],
                top: rect[1],
                right: rect[0] + rect[2].max(0),
                bottom: rect[1] + rect[3].max(0),
            };
            unsafe {
                self.controller
                    .SetBounds(bounds)
                    .map_err(|error| format!("WebView2 bounds failed: {error}"))?;
            }
            self.rect = Some(rect);
            Ok(())
        }

        fn show(&mut self, visible: bool) -> Result<(), String> {
            if self.visible == visible {
                return Ok(());
            }
            unsafe {
                self.controller
                    .SetIsVisible(visible)
                    .map_err(|error| format!("WebView2 visibility failed: {error}"))?;
            }
            self.visible = visible;
            Ok(())
        }

        fn set_scripts_enabled(&mut self, allow_scripts: bool) -> Result<(), String> {
            if self.allow_scripts == Some(allow_scripts) {
                return Ok(());
            }
            unsafe {
                self.webview
                    .Settings()
                    .map_err(|error| format!("WebView2 settings failed: {error}"))?
                    .SetIsScriptEnabled(allow_scripts)
                    .map_err(|error| format!("WebView2 script setting failed: {error}"))?;
            }
            self.allow_scripts = Some(allow_scripts);
            Ok(())
        }

        fn navigate(&mut self, source: &ReportSource) -> Result<(), String> {
            match source {
                ReportSource::Url(url) => {
                    let url = CoTaskMemPWSTR::from(url.as_str());
                    unsafe {
                        self.webview
                            .Navigate(*url.as_ref().as_pcwstr())
                            .map_err(|error| format!("WebView2 navigation failed: {error}"))?;
                    }
                }
                ReportSource::Html(html) => {
                    let html = CoTaskMemPWSTR::from(html.as_str());
                    unsafe {
                        self.webview
                            .NavigateToString(*html.as_ref().as_pcwstr())
                            .map_err(|error| {
                                format!("WebView2 inline navigation failed: {error}")
                            })?;
                    }
                }
                ReportSource::Blocked(_) | ReportSource::Empty => {}
            }
            Ok(())
        }
    }

    impl Drop for HtmlReportView {
        fn drop(&mut self) {
            unsafe {
                let _ = self.controller.Close();
            }
        }
    }

    fn webview_user_data_dir(profile_generation: u32) -> Result<PathBuf, String> {
        if profile_generation > 0 {
            return ensure_webview_user_data_dir(recovery_webview_user_data_dir(
                profile_generation,
            ));
        }

        if let Some(path) = std::env::var_os("DRAGONGUI_HTMLREPORT_USER_DATA_DIR") {
            return ensure_webview_user_data_dir(PathBuf::from(path));
        }

        let dir = if std::env::var_os("DRAGONGUI_SMOKE_FRAMES").is_some() {
            std::env::current_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .join(".dragongui-webview2")
        } else if let Some(path) = std::env::var_os("LOCALAPPDATA") {
            PathBuf::from(path).join("DragonGUI").join("WebView2")
        } else {
            std::env::temp_dir().join("DragonGUI").join("WebView2")
        };

        ensure_webview_user_data_dir(dir)
    }

    fn recovery_webview_user_data_dir(profile_generation: u32) -> PathBuf {
        std::env::temp_dir().join("DragonGUI").join(format!(
            "WebView2-recovery-{}-{}",
            process::id(),
            profile_generation
        ))
    }

    fn ensure_webview_user_data_dir(path: PathBuf) -> Result<PathBuf, String> {
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "failed to create WebView2 user data dir {}: {error}",
                path.display()
            )
        })?;
        Ok(path)
    }

    fn is_webview_initialization_error(error: &str) -> bool {
        error.starts_with("WebView2 environment failed:")
            || error.starts_with("WebView2 controller failed:")
    }

    fn should_retry_with_fresh_profile(error: &str) -> bool {
        error.starts_with("WebView2 controller failed:")
    }

    fn hwnd_from_window(window: &Window) -> Option<HWND> {
        let handle = window.window_handle().ok()?.as_raw();
        match handle {
            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
            _ => None,
        }
    }

    fn collect_visible_reports(
        node: &WidgetNode,
        layout: &LayoutResult,
        out: &mut Vec<(String, ReportSource, [i32; 4], bool)>,
    ) {
        if node.kind == WidgetKind::HtmlReport {
            if let Some(rect) = layout
                .visible_rect(&node.id)
                .or_else(|| layout.rects.get(&node.id).copied())
            {
                if let Some(bounds) = webview_bounds(rect) {
                    out.push((
                        node.id.clone(),
                        source_for_node(node),
                        bounds,
                        node.props.html_report_allow_scripts,
                    ));
                }
            }
        }
        for child in &node.children {
            collect_visible_reports(child, layout, out);
        }
    }

    fn webview_bounds(rect: Rect) -> Option<[i32; 4]> {
        let w = rect.w.round().max(0.0) as i32;
        let h = rect.h.round().max(0.0) as i32;
        if w < 8 || h < 8 {
            return None;
        }
        Some([rect.x.round() as i32, rect.y.round() as i32, w, h])
    }

    fn source_for_node(node: &WidgetNode) -> ReportSource {
        if let Some(path) = node.props.html_report_path.as_deref() {
            return source_for_path(path, node.props.html_report_allow_remote);
        }
        if let Some(html) = node.props.html_report_html.as_deref() {
            if html.trim().is_empty() {
                ReportSource::Empty
            } else {
                ReportSource::Html(html.to_string())
            }
        } else {
            ReportSource::Empty
        }
    }

    fn source_for_path(path: &str, allow_remote: bool) -> ReportSource {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return ReportSource::Empty;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            return if allow_remote {
                ReportSource::Url(trimmed.to_string())
            } else {
                ReportSource::Blocked("remote URLs require allow_remote=True".to_string())
            };
        }
        if lower.starts_with("file://") {
            return ReportSource::Url(trimmed.to_string());
        }
        ReportSource::Url(path_to_file_url(trimmed))
    }

    fn path_to_file_url(path: &str) -> String {
        let path = Path::new(path);
        let absolute: PathBuf = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        };
        let path = absolute.to_string_lossy().replace('\\', "/");
        let encoded = path
            .chars()
            .map(|ch| match ch {
                ' ' => "%20".to_string(),
                '#' => "%23".to_string(),
                '?' => "%3F".to_string(),
                '%' => "%25".to_string(),
                _ => ch.to_string(),
            })
            .collect::<String>();
        format!("file:///{encoded}")
    }

    fn status_for_source(source: &ReportSource) -> String {
        match source {
            ReportSource::Url(url) => format!("loaded {url}"),
            ReportSource::Html(html) => format!("loaded inline HTML ({} bytes)", html.len()),
            ReportSource::Blocked(reason) => format!("blocked: {reason}"),
            ReportSource::Empty => "no report source".to_string(),
        }
    }

    fn source_label(source: Option<&ReportSource>) -> &'static str {
        match source {
            Some(ReportSource::Url(_)) => "url",
            Some(ReportSource::Html(_)) => "html",
            Some(ReportSource::Blocked(_)) => "blocked",
            Some(ReportSource::Empty) => "empty",
            None => "none",
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use serde_json::{json, Value};
    use winit::window::Window;

    use crate::{document::WidgetNode, layout::LayoutResult};

    pub(crate) struct PlatformHtmlReportWebViewManager;

    impl PlatformHtmlReportWebViewManager {
        pub(crate) fn new(_window: &Window) -> Self {
            Self
        }

        pub(crate) fn sync(&mut self, _tree: Option<&WidgetNode>, _layout: Option<&LayoutResult>) {}

        pub(crate) fn hide_all(&mut self) {}

        pub(crate) fn snapshot(&self) -> Value {
            json!({
                "platform": "unsupported",
                "enabled": false,
                "reason": "embedded HTML reports currently require Windows WebView2",
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn unsupported_snapshot_is_stable() {
            let manager = PlatformHtmlReportWebViewManager;
            let snapshot = manager.snapshot();

            assert_eq!(snapshot["platform"], "unsupported");
            assert_eq!(snapshot["enabled"], false);
            assert!(snapshot["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("WebView2"));
        }
    }
}
