use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::capture::{capture_foreground, capture_selected_text, paste_text};
use crate::config::AppConfig;
use crate::hotkey::{HotkeyAction, HotkeyManager};
use crate::translator::{benchmark_all_engines, parse_target_tag, translate_with_fallback, EngineBenchmark, TranslationResult};
use crate::tray::{show_and_focus_window, SystemTray, TrayAction};

pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Tự động phát hiện"),
    ("vi", "Tiếng Việt"),
    ("en", "Tiếng Anh"),
    ("zh", "Tiếng Trung (Giản thể)"),
    ("ja", "Tiếng Nhật"),
    ("ko", "Tiếng Hàn"),
    ("fr", "Tiếng Pháp"),
    ("de", "Tiếng Đức"),
    ("ru", "Tiếng Nga"),
    ("es", "Tiếng Tây Ban Nha"),
    ("th", "Tiếng Thái"),
];

pub const ENGINES: &[(&str, &str)] = &[
    ("google_pa", "Google Translate (PA)"),
    ("microsoft", "Microsoft Translator"),
    ("google", "Google Translate (GTX)"),
    ("bing", "Bing Dict"),
    ("modernmt", "ModernMT"),
    ("papago", "Naver Papago"),
    ("yandex", "Yandex Translate"),
    ("translate_com", "Translate.com"),
    ("mymemory", "MyMemory"),
    ("deepl", "DeepL (Web / Free)"),
];

pub enum AppEvent {
    Hotkey(HotkeyAction),
    Tray(TrayAction),
    TranslationDone {
        result: Result<TranslationResult, String>,
        latency_ms: u64,
        is_tooltip: bool,
    },
    BenchmarkDone(Vec<EngineBenchmark>),
}

#[derive(PartialEq)]
pub enum ViewMode {
    Translator,
    Settings,
}

pub struct QuickTranApp {
    pub config: AppConfig,
    pub view_mode: ViewMode,

    // Translation fields
    pub source_text: String,
    pub target_text: String,
    pub current_engine_badge: String,
    pub fallback_notice: Option<String>,
    pub last_latency_ms: Option<u64>,
    pub is_translating: bool,
    pub toast_message: Option<(String, Instant)>,

    // Tooltip popup state
    pub show_tooltip: bool,
    pub tooltip_source: String,
    pub tooltip_target: String,
    pub tooltip_engine: String,
    pub tooltip_latency: Option<u64>,
    pub tooltip_timer: Instant,

    // Benchmark state
    pub is_benchmarking: bool,
    pub benchmark_results: Vec<EngineBenchmark>,

    // Windows source tracking
    pub source_hwnd: Option<isize>,

    // Event channels
    pub tx: Sender<AppEvent>,
    pub rx: Receiver<AppEvent>,

    // Tray & Hotkey references to keep them alive
    pub _tray: Option<SystemTray>,
    pub _hotkey: Option<HotkeyManager>,
}

impl QuickTranApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);
        setup_dark_theme(&cc.egui_ctx);

        let config = AppConfig::load();
        let (tx, rx) = channel::<AppEvent>();

        // System Tray
        let (tray_internal_tx, tray_internal_rx) = channel::<TrayAction>();
        let tray = SystemTray::new(tray_internal_tx, cc.egui_ctx.clone());

        let tray_bridge_tx = tx.clone();
        let tray_ctx_clone = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(action) = tray_internal_rx.recv() {
                let _ = tray_bridge_tx.send(AppEvent::Tray(action));
                tray_ctx_clone.request_repaint();
            }
        });

        // Hotkey manager
        let hotkey_tx = tx.clone();
        let (hk_internal_tx, hk_internal_rx) = channel::<HotkeyAction>();
        let hotkey_mgr = HotkeyManager::start(
            hk_internal_tx,
            config.hotkey_translate.clone(),
            config.hotkey_tooltip.clone(),
        );

        // Bridge hotkey thread to app events
        let bridge_tx = hotkey_tx.clone();
        let ctx_clone = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(action) = hk_internal_rx.recv() {
                let _ = bridge_tx.send(AppEvent::Hotkey(action));
                ctx_clone.request_repaint();
            }
        });

        Self {
            config,
            view_mode: ViewMode::Translator,
            source_text: String::new(),
            target_text: String::new(),
            current_engine_badge: String::new(),
            fallback_notice: None,
            last_latency_ms: None,
            is_translating: false,
            toast_message: None,

            show_tooltip: false,
            tooltip_source: String::new(),
            tooltip_target: String::new(),
            tooltip_engine: String::new(),
            tooltip_latency: None,
            tooltip_timer: Instant::now(),

            is_benchmarking: false,
            benchmark_results: Vec::new(),

            source_hwnd: None,

            tx,
            rx,
            _tray: tray,
            _hotkey: Some(hotkey_mgr),
        }
    }

    pub fn trigger_translate(&mut self, is_tooltip: bool) {
        let text = if is_tooltip {
            self.tooltip_source.trim().to_string()
        } else {
            self.source_text.trim().to_string()
        };

        if text.is_empty() {
            return;
        }

        self.is_translating = true;
        let (actual_target, clean_text) = parse_target_tag(&text, &self.config.target_lang);
        let provider = self.config.provider.clone();
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            let start = Instant::now();
            let res = translate_with_fallback(&clean_text, &actual_target, &provider);
            let latency_ms = start.elapsed().as_millis() as u64;

            let _ = tx.send(AppEvent::TranslationDone {
                result: res,
                latency_ms,
                is_tooltip,
            });
        });
    }

    pub fn trigger_benchmark(&mut self) {
        if self.is_benchmarking {
            return;
        }
        self.is_benchmarking = true;
        self.benchmark_results.clear();

        let target = self.config.target_lang.clone();
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            let results = benchmark_all_engines("Hello world", &target);
            let _ = tx.send(AppEvent::BenchmarkDone(results));
        });
    }

    pub fn set_toast(&mut self, msg: &str) {
        self.toast_message = Some((msg.to_string(), Instant::now()));
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                AppEvent::Hotkey(action) => match action {
                    HotkeyAction::Translate => {
                        self.source_hwnd = capture_foreground();
                        if let Some(text) = capture_selected_text() {
                            self.source_text = text;
                            self.trigger_translate(false);
                        }
                        if !self.config.auto_overwrite {
                            show_and_focus_window();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                    }
                    HotkeyAction::Tooltip => {
                        self.source_hwnd = capture_foreground();
                        if let Some(text) = capture_selected_text() {
                            self.tooltip_source = text;
                            self.tooltip_target = "Đang dịch...".to_string();
                            self.show_tooltip = true;
                            self.tooltip_timer = Instant::now();
                            self.trigger_translate(true);
                        }
                    }
                },
                AppEvent::TranslationDone {
                    result,
                    latency_ms,
                    is_tooltip,
                } => {
                    self.is_translating = false;
                    match result {
                        Ok(res) => {
                            if is_tooltip {
                                self.tooltip_target = res.text.clone();
                                self.tooltip_engine = res.engine.clone();
                                self.tooltip_latency = Some(latency_ms);
                                self.tooltip_timer = Instant::now();
                            } else {
                                self.target_text = res.text.clone();
                                self.current_engine_badge = res.engine.clone();
                                self.fallback_notice = res.fallback_from;
                                self.last_latency_ms = Some(latency_ms);

                                if self.config.auto_copy {
                                    if let Ok(mut clip) = arboard::Clipboard::new() {
                                        let _ = clip.set_text(res.text.clone());
                                        self.set_toast("Đã tự động sao chép vào bộ nhớ tạm");
                                    }
                                }

                                if self.config.auto_overwrite {
                                    paste_text(&res.text, self.source_hwnd);
                                    self.set_toast("Đã tự động ghi đè văn bản gốc");
                                }
                            }
                        }
                        Err(err) => {
                            if is_tooltip {
                                self.tooltip_target = format!("Lỗi: {}", err);
                            } else {
                                self.target_text = format!("Lỗi dịch: {}", err);
                                self.last_latency_ms = Some(latency_ms);
                            }
                        }
                    }
                }
                AppEvent::Tray(action) => match action {
                    TrayAction::OpenTranslator => {
                        self.view_mode = ViewMode::Translator;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                    TrayAction::OpenSettings => {
                        self.view_mode = ViewMode::Settings;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                },
                AppEvent::BenchmarkDone(results) => {
                    self.is_benchmarking = false;
                    self.benchmark_results = results;
                }
            }
        }
    }
}

impl eframe::App for QuickTranApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events(ctx);

        // Intercept close button if close_to_tray is enabled
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.config.close_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Hide on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.view_mode == ViewMode::Settings {
                self.view_mode = ViewMode::Translator;
            } else if self.config.close_to_tray {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Ctrl + Enter to translate
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
            self.trigger_translate(false);
        }

        // Render Tooltip Viewport (Secondary popup)
        if self.show_tooltip {
            if self.tooltip_timer.elapsed() > Duration::from_secs(8) {
                self.show_tooltip = false;
            } else {
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("quicktran_tooltip"),
                    egui::ViewportBuilder::default()
                        .with_title("QuickTran Preview")
                        .with_inner_size([380.0, 160.0])
                        .with_always_on_top()
                        .with_decorations(false)
                        .with_transparent(true),
                    |ctx, _class| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE
                                .fill(egui::Color32::from_rgb(26, 27, 38))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(122, 162, 247)))
                                .corner_radius(10.0)
                                .inner_margin(12.0))
                            .show(ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(egui::Color32::from_rgb(122, 162, 247), "⚡ QuickTran Tooltip");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("✕").clicked() {
                                            // Close tooltip
                                        }
                                    });
                                });

                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(&self.tooltip_source).italics().weak().size(11.0));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(&self.tooltip_target).strong().size(14.0));

                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if let Some(lat) = self.tooltip_latency {
                                        ui.label(egui::RichText::new(format!("⏱ {}ms • {}", lat, self.tooltip_engine)).size(10.0).weak());
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("📋 Sao chép").clicked() {
                                            if let Ok(mut clip) = arboard::Clipboard::new() {
                                                let _ = clip.set_text(self.tooltip_target.clone());
                                            }
                                        }
                                    });
                                });
                            });
                    },
                );
            }
        }

        // Render Main Window
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(18, 19, 26))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                render_header(self, ui, ctx);
                ui.add_space(6.0);

                match self.view_mode {
                    ViewMode::Translator => render_translator_view(self, ui),
                    ViewMode::Settings => render_settings_view(self, ui),
                }

                // Render toast notification if active
                if let Some((msg, time)) = &self.toast_message {
                    if time.elapsed() < Duration::from_secs(3) {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("✓ {}", msg))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(110, 231, 183)),
                            );
                        });
                    }
                }
            });
    }
}

fn render_header(app: &mut QuickTranApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("QuickTran")
                .strong()
                .size(15.0)
                .color(egui::Color32::from_rgb(96, 165, 250)),
        );
        ui.label(
            egui::RichText::new("Native")
                .size(10.0)
                .color(egui::Color32::from_rgb(147, 197, 253)),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let hide_btn = egui::Button::new(egui::RichText::new("Ẩn").size(12.0));
            if ui.add_sized([42.0, 26.0], hide_btn).on_hover_text("Thu nhỏ vào khay hệ thống").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }

            let btn_label = if app.view_mode == ViewMode::Settings {
                "← Quay lại"
            } else {
                "⚙ Cài đặt"
            };

            let set_btn = egui::Button::new(egui::RichText::new(btn_label).size(12.0));
            if ui.add_sized([78.0, 26.0], set_btn).clicked() {
                app.view_mode = if app.view_mode == ViewMode::Settings {
                    ViewMode::Translator
                } else {
                    ViewMode::Settings
                };
            }

            // Compact engine selector on top-right, aligned with buttons
            egui::ComboBox::from_id_salt("provider_select_top")
                .selected_text(
                    egui::RichText::new(engine_short_name(&app.config.provider))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(203, 213, 225)),
                )
                .width(115.0)
                .show_ui(ui, |ui| {
                    for (id, name) in ENGINES {
                        if ui.selectable_value(&mut app.config.provider, id.to_string(), *name).clicked() {
                            app.config.save();
                        }
                    }
                });
        });
    });
}

fn render_translator_view(app: &mut QuickTranApp, ui: &mut egui::Ui) {
    // Symmetrical language switcher with matching heights
    ui.horizontal(|ui| {
        let total_w = ui.available_width();
        let swap_w = 34.0;
        let spacing = ui.spacing().item_spacing.x;
        let combo_w = ((total_w - swap_w - spacing * 2.0) / 2.0).max(110.0);
        let row_h = 28.0;

        egui::ComboBox::from_id_salt("src_lang")
            .selected_text(
                egui::RichText::new(lang_name(&app.config.source_lang))
                    .size(13.0)
                    .color(egui::Color32::from_rgb(226, 232, 240)),
            )
            .width(combo_w)
            .show_ui(ui, |ui| {
                for (code, name) in LANGUAGES {
                    if ui.selectable_value(&mut app.config.source_lang, code.to_string(), *name).clicked() {
                        app.config.save();
                    }
                }
            });

        let swap_btn = egui::Button::new(
            egui::RichText::new("⇄")
                .size(14.0)
                .color(egui::Color32::from_rgb(203, 213, 225)),
        );
        if ui.add_sized([swap_w, row_h], swap_btn).on_hover_text("Đổi chiều ngôn ngữ").clicked() {
            if app.config.source_lang != "auto" {
                std::mem::swap(&mut app.config.source_lang, &mut app.config.target_lang);
                app.config.save();
            }
        }

        egui::ComboBox::from_id_salt("tgt_lang")
            .selected_text(
                egui::RichText::new(lang_name(&app.config.target_lang))
                    .size(13.0)
                    .color(egui::Color32::from_rgb(226, 232, 240)),
            )
            .width(combo_w)
            .show_ui(ui, |ui| {
                for (code, name) in LANGUAGES {
                    if *code == "auto" {
                        continue;
                    }
                    if ui.selectable_value(&mut app.config.target_lang, code.to_string(), *name).clicked() {
                        app.config.save();
                    }
                }
            });
    });

    ui.add_space(6.0);

    // Source input section
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("VĂN BẢN GỐC")
                .size(10.0)
                .strong()
                .color(egui::Color32::from_rgb(148, 163, 184)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.source_text.is_empty() {
                let clear_btn = egui::Button::new(egui::RichText::new("Xóa").size(11.0));
                if ui.add_sized([44.0, 22.0], clear_btn).clicked() {
                    app.source_text.clear();
                }
            }
            let char_count = app.source_text.chars().count();
            if char_count > 0 {
                ui.label(egui::RichText::new(format!("{} ký tự", char_count)).size(10.0).weak());
            }
        });
    });

    ui.add(
        egui::TextEdit::multiline(&mut app.source_text)
            .hint_text("Nhập hoặc bôi đen văn bản ngoài ứng dụng rồi bấm Alt+T...")
            .margin(egui::Margin::symmetric(8, 6))
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );

    ui.add_space(4.0);

    // Middle action bar
    ui.horizontal(|ui| {
        if app.is_translating {
            ui.spinner();
            ui.label(
                egui::RichText::new("Đang dịch...")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(96, 165, 250)),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let btn = egui::Button::new(
                egui::RichText::new("⚡ Dịch (Ctrl+Enter)")
                    .strong()
                    .size(11.0)
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(37, 99, 235));
            if ui.add_sized([135.0, 26.0], btn).clicked() {
                app.trigger_translate(false);
            }
        });
    });

    ui.add_space(4.0);

    // Target result section
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("BẢN DỊCH")
                .size(10.0)
                .strong()
                .color(egui::Color32::from_rgb(148, 163, 184)),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !app.target_text.is_empty() {
                let paste_btn = egui::Button::new(egui::RichText::new("✍ Ghi đè").size(11.0));
                if ui.add_sized([72.0, 22.0], paste_btn).on_hover_text("Dán bản dịch đè vào ứng dụng gốc").clicked() {
                    paste_text(&app.target_text, app.source_hwnd);
                    app.set_toast("Đã ghi đè vào ứng dụng gốc");
                }

                let copy_btn = egui::Button::new(egui::RichText::new("📋 Sao chép").size(11.0));
                if ui.add_sized([78.0, 22.0], copy_btn).clicked() {
                    if let Ok(mut clip) = arboard::Clipboard::new() {
                        let _ = clip.set_text(app.target_text.clone());
                        app.set_toast("Đã sao chép vào bộ nhớ tạm");
                    }
                }
            }
        });
    });

    ui.add(
        egui::TextEdit::multiline(&mut app.target_text)
            .margin(egui::Margin::symmetric(8, 6))
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );

    // Status / Metainfo bar
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if let Some(lat) = app.last_latency_ms {
            ui.label(
                egui::RichText::new(format!("⏱ {}ms", lat))
                    .size(11.0)
                    .color(egui::Color32::from_rgb(110, 231, 183)),
            );
            ui.label(egui::RichText::new("•").size(11.0).weak());
        }

        if !app.current_engine_badge.is_empty() {
            ui.label(
                egui::RichText::new(&app.current_engine_badge)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(147, 197, 253)),
            );
        }

        if let Some(fb) = &app.fallback_notice {
            ui.label(
                egui::RichText::new(format!("(chuyển từ {})", fb))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(251, 191, 36)),
            );
        }
    });
}

fn render_settings_view(app: &mut QuickTranApp, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Cài đặt");
        ui.add_space(6.0);

        // Group 1: General options
        ui.group(|ui| {
            ui.strong("Tùy chọn tương tác");
            ui.add_space(4.0);

            if ui.checkbox(&mut app.config.auto_fallback, "Tự động chuyển engine khác khi lỗi / 429").changed() {
                app.config.save();
            }
            if ui.checkbox(&mut app.config.auto_overwrite, "Tự động ghi đè văn bản gốc khi bấm phím tắt dịch").changed() {
                app.config.save();
            }
            if ui.checkbox(&mut app.config.auto_copy, "Tự động sao chép kết quả dịch vào clipboard").changed() {
                app.config.save();
            }
            if ui.checkbox(&mut app.config.close_to_tray, "Thu nhỏ vào khay hệ thống (Tray) khi đóng cửa sổ").changed() {
                app.config.save();
            }
        });

        ui.add_space(8.0);

        // Group 2: Hotkeys
        ui.group(|ui| {
            ui.strong("Phím tắt toàn hệ thống");
            ui.add_space(4.0);
            ui.label(format!("• Dịch chính: {}", app.config.hotkey_translate));
            ui.label(format!("• Tooltip nổi: {}", app.config.hotkey_tooltip));
            ui.weak("(Phím tắt mặc định Alt+T để dịch / ghi đè, Alt+D để hiện tooltip)");
        });

        ui.add_space(8.0);

        // Group 3: Speed test / Benchmark
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong("Kiểm tra tốc độ phản hồi (Benchmark)");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.is_benchmarking {
                        ui.spinner();
                        ui.label("Đang test...");
                    } else if ui.button("⚡ Test tốc độ các engine").clicked() {
                        app.trigger_benchmark();
                    }
                });
            });

            ui.add_space(6.0);

            if !app.benchmark_results.is_empty() {
                // Find fastest
                let min_lat = app.benchmark_results.iter().filter(|b| b.success).map(|b| b.latency_ms).min();

                for item in &app.benchmark_results {
                    ui.horizontal(|ui| {
                        let is_active = app.config.provider == item.id;
                        let is_fastest = item.success && Some(item.latency_ms) == min_lat;

                        ui.label(egui::RichText::new(&item.name).strong());

                        if is_fastest {
                            ui.label(
                                egui::RichText::new("⚡ Nhanh nhất")
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(158, 206, 106)),
                            );
                        }

                        if is_active {
                            ui.label(
                                egui::RichText::new("Đang dùng")
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(122, 162, 247)),
                            );
                        }

                        if item.success {
                            ui.label(
                                egui::RichText::new(format!("{}ms", item.latency_ms))
                                    .color(egui::Color32::from_rgb(158, 206, 106)),
                            );
                        } else {
                            let err_msg = item.error.clone().unwrap_or_else(|| "Lỗi".to_string());
                            ui.label(
                                egui::RichText::new(format!("Lỗi: {}", err_msg))
                                    .color(egui::Color32::from_rgb(247, 118, 142)),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if item.success && !is_active {
                                if ui.small_button("Chọn").clicked() {
                                    app.config.provider = item.id.clone();
                                    app.config.save();
                                }
                            }
                        });
                    });
                    ui.separator();
                }
            } else {
                ui.weak("Bấm nút trên để kiểm tra tốc độ phản hồi của tất cả 10 engine miễn phí cùng lúc.");
            }
        });
    });
}

fn lang_name(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| code.to_string())
}

#[allow(dead_code)]
fn engine_name(id: &str) -> String {
    ENGINES
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| id.to_string())
}

fn engine_short_name(id: &str) -> &'static str {
    match id {
        "google_pa" => "Google (PA)",
        "microsoft" => "Microsoft",
        "google" => "Google (GTX)",
        "bing" => "Bing",
        "modernmt" => "ModernMT",
        "papago" => "Papago",
        "yandex" => "Yandex",
        "translate_com" => "Translate.com",
        "mymemory" => "MyMemory",
        "deepl" => "DeepL",
        _ => "Engine",
    }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Primary font (Segoe UI / Arial)
    if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf")
        .or_else(|_| std::fs::read("C:\\Windows\\Fonts\\arial.ttf"))
    {
        fonts.font_data.insert(
            "system_font".to_owned(),
            Arc::new(egui::FontData::from_owned(font_data)),
        );
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, "system_font".to_owned());
        }
    }

    // Windows Symbol font (fixes missing symbols: ⇄, ⚙, etc.)
    if let Ok(sym_data) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
        fonts.font_data.insert(
            "segoe_symbol".to_owned(),
            Arc::new(egui::FontData::from_owned(sym_data)),
        );
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.push("segoe_symbol".to_owned());
        }
    }

    // Windows Emoji font (fixes missing emojis: ⚡, 📋, ✍, etc.)
    if let Ok(emj_data) = std::fs::read("C:\\Windows\\Fonts\\seguiemj.ttf") {
        fonts.font_data.insert(
            "segoe_emoji".to_owned(),
            Arc::new(egui::FontData::from_owned(emj_data)),
        );
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.push("segoe_emoji".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

fn setup_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_corner_radius = 8.0.into();

    let bg = egui::Color32::from_rgb(18, 19, 26);
    let card_bg = egui::Color32::from_rgb(26, 28, 38);
    let border = egui::Color32::from_rgb(46, 50, 68);
    let text = egui::Color32::from_rgb(235, 240, 248);

    visuals.panel_fill = bg;
    visuals.window_fill = bg;
    visuals.override_text_color = Some(text);

    // Frame styling
    visuals.widgets.noninteractive.bg_fill = card_bg;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.noninteractive.corner_radius = 6.0.into();
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text);

    // Inactive buttons / dropdowns
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 33, 46);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.inactive.corner_radius = 6.0.into();
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(203, 213, 225));

    // Hovered buttons
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(42, 47, 66);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(79, 88, 122));
    visuals.widgets.hovered.corner_radius = 6.0.into();
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    // Active / Clicked buttons
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(37, 99, 235);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(59, 130, 246));
    visuals.widgets.active.corner_radius = 6.0.into();
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

    visuals.selection.bg_fill = egui::Color32::from_rgb(37, 99, 235);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.interact_size = egui::vec2(32.0, 26.0); // Standard baseline height
    style.spacing.button_padding = egui::vec2(8.0, 4.0); // Balanced padding
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.combo_width = 110.0;
    ctx.set_style(style);
}
