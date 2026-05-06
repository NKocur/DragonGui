#[cfg(not(windows))]
fn main() {
    println!("WEBVIEW2_PROBE skipped: Windows-only probe");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use winit::event_loop::EventLoop;

    let event_loop = EventLoop::new()?;
    let mut app = platform::ProbeApp::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, fs, path::PathBuf, sync::mpsc, sync::Arc};

    use webview2_com::{
        CoTaskMemPWSTR, CoreWebView2EnvironmentOptions,
        CreateCoreWebView2ControllerCompletedHandler,
        CreateCoreWebView2EnvironmentCompletedHandler,
        Microsoft::Web::WebView2::Win32::{
            CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
            ICoreWebView2EnvironmentOptions,
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
        application::ApplicationHandler,
        dpi::{LogicalSize, PhysicalSize},
        event::WindowEvent,
        event_loop::ActiveEventLoop,
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
        window::{Window, WindowId},
    };

    pub(crate) struct ProbeApp {
        window: Option<Arc<Window>>,
        wgpu: Option<MiniWgpu>,
        controller: Option<ICoreWebView2Controller>,
        webview: Option<ICoreWebView2>,
        attempted: bool,
        exit_after_init: bool,
        wgpu_first: bool,
    }

    impl Default for ProbeApp {
        fn default() -> Self {
            Self {
                window: None,
                wgpu: None,
                controller: None,
                webview: None,
                attempted: false,
                exit_after_init: std::env::var_os("DRAGONGUI_WEBVIEW2_PROBE_EXIT_AFTER_INIT")
                    .is_some(),
                wgpu_first: std::env::var_os("DRAGONGUI_WEBVIEW2_PROBE_WGPU_FIRST").is_some(),
            }
        }
    }

    impl ApplicationHandler for ProbeApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }

            let attrs = Window::default_attributes()
                .with_title("DragonGUI WebView2 standalone probe")
                .with_inner_size(LogicalSize::new(900.0, 560.0));
            match event_loop.create_window(attrs) {
                Ok(window) => {
                    println!("WEBVIEW2_PROBE window_created");
                    let window = Arc::new(window);
                    window.request_redraw();
                    self.window = Some(window);
                }
                Err(error) => {
                    eprintln!("WEBVIEW2_PROBE window_error: {error}");
                    event_loop.exit();
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    if !self.attempted {
                        self.attempted = true;
                        self.initialize_webview(event_loop);
                    }
                }
                WindowEvent::Resized(size) => self.resize(size),
                _ => {}
            }
        }
    }

    impl ProbeApp {
        fn initialize_webview(&mut self, event_loop: &ActiveEventLoop) {
            let Some(window) = self.window.as_ref() else {
                eprintln!("WEBVIEW2_PROBE error: missing window");
                event_loop.exit();
                return;
            };

            if self.wgpu_first {
                match pollster::block_on(MiniWgpu::new(Arc::clone(window))) {
                    Ok(mut wgpu) => {
                        if let Err(error) = wgpu.render_clear() {
                            eprintln!("WEBVIEW2_PROBE wgpu_error: {error}");
                            event_loop.exit();
                            return;
                        }
                        println!("WEBVIEW2_PROBE wgpu_frame_presented");
                        self.wgpu = Some(wgpu);
                    }
                    Err(error) => {
                        eprintln!("WEBVIEW2_PROBE wgpu_error: {error}");
                        event_loop.exit();
                        return;
                    }
                }
            }

            match create_webview(window) {
                Ok(created) => {
                    println!("WEBVIEW2_PROBE controller_ready");
                    self.controller = Some(created.controller);
                    self.webview = Some(created.webview);
                    if self.exit_after_init {
                        event_loop.exit();
                    }
                }
                Err(error) => {
                    eprintln!("WEBVIEW2_PROBE error: {error}");
                    event_loop.exit();
                }
            }
        }

        fn resize(&self, size: PhysicalSize<u32>) {
            let Some(controller) = self.controller.as_ref() else {
                return;
            };
            let bounds = RECT {
                left: 0,
                top: 0,
                right: size.width as i32,
                bottom: size.height as i32,
            };
            unsafe {
                let _ = controller.SetBounds(bounds);
            }
        }
    }

    struct MiniWgpu {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
    }

    impl MiniWgpu {
        async fn new(window: Arc<Window>) -> Result<Self, String> {
            let size = window.inner_size();
            let width = size.width.max(1);
            let height = size.height.max(1);
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let surface = instance
                .create_surface(window)
                .map_err(|error| format!("surface failed: {error}"))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|error| format!("adapter failed: {error}"))?;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("webview2-standalone-probe"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    experimental_features: wgpu::ExperimentalFeatures::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: Default::default(),
                })
                .await
                .map_err(|error| format!("device failed: {error}"))?;
            let config = surface
                .get_default_config(&adapter, width, height)
                .ok_or_else(|| "unsupported surface format".to_string())?;
            surface.configure(&device, &config);
            Ok(Self {
                surface,
                device,
                queue,
                config,
            })
        }

        fn render_clear(&mut self) -> Result<(), String> {
            let texture = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    self.surface.configure(&self.device, &self.config);
                    texture
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.device, &self.config);
                    return Err("surface texture unavailable after reconfigure".to_string());
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    return Err("surface texture timed out".to_string());
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    return Err("surface texture occluded".to_string());
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    return Err("surface texture validation failed".to_string());
                }
            };
            let view = texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("webview2-standalone-probe-clear"),
                });
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("webview2-standalone-probe-clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.04,
                                g: 0.06,
                                b: 0.10,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            self.queue.submit(std::iter::once(encoder.finish()));
            texture.present();
            Ok(())
        }
    }

    struct CreatedWebView {
        controller: ICoreWebView2Controller,
        webview: ICoreWebView2,
    }

    fn create_webview(window: &Window) -> Result<CreatedWebView, String> {
        let hwnd = hwnd_from_window(window).ok_or("window does not expose Win32 HWND")?;
        println!("WEBVIEW2_PROBE hwnd={:?}", hwnd.0);

        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| format!("COM init failed: {error}"))?;
        }

        let user_data_dir = user_data_dir()?;
        println!("WEBVIEW2_PROBE user_data_dir={}", user_data_dir.display());
        let environment = create_environment(user_data_dir)?;
        println!("WEBVIEW2_PROBE environment_ready");
        let controller = create_controller(&environment, hwnd)?;
        println!("WEBVIEW2_PROBE controller_created");

        let size = window.inner_size();
        let bounds = RECT {
            left: 0,
            top: 0,
            right: size.width as i32,
            bottom: size.height as i32,
        };
        unsafe {
            controller
                .SetBounds(bounds)
                .map_err(|error| format!("controller bounds failed: {error}"))?;
            controller
                .SetIsVisible(true)
                .map_err(|error| format!("controller visibility failed: {error}"))?;
        }

        let webview = unsafe {
            controller
                .CoreWebView2()
                .map_err(|error| format!("CoreWebView2 failed: {error}"))?
        };
        unsafe {
            if let Ok(settings) = webview.Settings() {
                let _ = settings.SetAreDefaultContextMenusEnabled(true);
                let _ = settings.SetAreDevToolsEnabled(true);
                let _ = settings.SetIsScriptEnabled(true);
            }
        }

        let html = CoTaskMemPWSTR::from(PROBE_HTML);
        unsafe {
            webview
                .NavigateToString(*html.as_ref().as_pcwstr())
                .map_err(|error| format!("NavigateToString failed: {error}"))?;
        }

        Ok(CreatedWebView {
            controller,
            webview,
        })
    }

    fn create_environment(
        user_data_dir: PathBuf,
    ) -> Result<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment, String>
    {
        let browser_folder = CoTaskMemPWSTR::from("");
        let user_data_text = user_data_dir.display().to_string();
        let user_data_folder = CoTaskMemPWSTR::from(user_data_text.as_str());
        let options: ICoreWebView2EnvironmentOptions =
            CoreWebView2EnvironmentOptions::default().into();
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
        .map_err(|error| format!("environment creation failed: {error}"))?;

        rx.recv()
            .map_err(|_| "environment callback channel failed".to_string())?
            .map_err(|error| format!("environment callback failed: {error}"))
    }

    fn create_controller(
        environment: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
        hwnd: HWND,
    ) -> Result<ICoreWebView2Controller, String> {
        let (tx, rx) = mpsc::channel();
        let environment = environment.clone();
        CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                environment
                    .CreateCoreWebView2Controller(hwnd, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                tx.send(controller.ok_or_else(|| WindowsError::from(E_POINTER)))
                    .map_err(|_| WindowsError::from(E_POINTER))?;
                Ok(())
            }),
        )
        .map_err(|error| format!("controller creation failed: {error}"))?;

        rx.recv()
            .map_err(|_| "controller callback channel failed".to_string())?
            .map_err(|error| format!("controller callback failed: {error}"))
    }

    fn user_data_dir() -> Result<PathBuf, String> {
        let dir = std::env::var_os("DRAGONGUI_WEBVIEW2_PROBE_USER_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join(".webview2-standalone-probe")
            });
        fs::create_dir_all(&dir).map_err(|error| {
            format!(
                "failed to create WebView2 user data dir {}: {error}",
                dir.display()
            )
        })?;
        Ok(dir)
    }

    fn hwnd_from_window(window: &Window) -> Option<HWND> {
        let handle = window.window_handle().ok()?.as_raw();
        match handle {
            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
            _ => None,
        }
    }

    const PROBE_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>WebView2 probe</title>
  <style>
    body {
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      font-family: Segoe UI, Arial, sans-serif;
      background: #101827;
      color: #edf3ff;
    }
    main {
      width: min(720px, calc(100vw - 64px));
      border: 1px solid rgba(255,255,255,.18);
      border-radius: 12px;
      padding: 28px;
      background: rgba(255,255,255,.06);
    }
    h1 { margin: 0 0 8px; }
    p { color: rgba(237,243,255,.72); }
    button {
      margin-top: 18px;
      padding: 10px 14px;
      border-radius: 6px;
      border: 1px solid rgba(255,255,255,.24);
      background: #22314c;
      color: #edf3ff;
    }
  </style>
</head>
<body>
  <main>
    <h1>WebView2 standalone probe</h1>
    <p>If this page is visible, WebView2 can create a controller in a plain winit window.</p>
    <button id="btn">Click probe</button>
    <p id="status">ready</p>
  </main>
  <script>
    document.getElementById("btn").addEventListener("click", () => {
      document.getElementById("status").textContent = "script event handled";
    });
  </script>
</body>
</html>"#;
}
