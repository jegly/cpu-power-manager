use gtk4::prelude::*;
use gtk4::{
    glib, AboutDialog, Application, ApplicationWindow, Box, Button, Entry, EventControllerMotion,
    Frame, GestureClick, Grid, HeaderBar, Label, LevelBar, License, MenuButton, Notebook,
    Orientation, Popover, Scale, ScrolledWindow, Separator, SpinButton, StringList, Switch, DropDown,
};
use crate::backend::{CpuManager, HwmonReader, PowerSupplyReader, RaplTracker};
use crate::backend::cpu::{CpuInfo, CpuDriver, CpuUsageTracker, PerCoreCpuUsageTracker};
use crate::backend::thermal::ThermalManager;
use crate::backend::profile::{Profile, ProfileManager};
use crate::config::{ConfigManager, set_autostart};
use std::sync::{Arc, Mutex};

pub struct AppWindow {
    window: ApplicationWindow,
    cpu_manager: Arc<Mutex<CpuManager>>,
    thermal_manager: Arc<Mutex<ThermalManager>>,
    profile_manager: Arc<Mutex<ProfileManager>>,
    config_manager: Arc<Mutex<ConfigManager>>,
    cpu_info: CpuInfo,
    // Dashboard labels
    freq_label: Label,
    usage_label: Label,
    temp_label: Label,
    governor_label: Label,
    turbo_label: Label,
    power_label: Label,
    battery_label: Label,
    fan_label: Label,
    active_profile_label: Label,
    // Graph
    cpu_usage_area: gtk4::DrawingArea,
    cpu_usage_history: Arc<Mutex<Vec<f32>>>,
    cpu_usage_tracker: Arc<Mutex<CpuUsageTracker>>,
    per_core_tracker: Arc<Mutex<PerCoreCpuUsageTracker>>,
    rapl_tracker: Arc<Mutex<RaplTracker>>,
    // Per-core panel container
    per_core_box: Box,
    // Last AC status for auto-switch detection
    last_ac_status: Arc<Mutex<Option<bool>>>,
}

impl AppWindow {
    pub fn new(app: &Application) -> Self {
        let config_manager = Arc::new(Mutex::new(
            ConfigManager::new().unwrap_or_else(|_| ConfigManager::new().unwrap()),
        ));
        let cpu_manager = Arc::new(Mutex::new(
            CpuManager::new().expect("Failed to initialize CPU manager"),
        ));
        let thermal_manager = Arc::new(Mutex::new(
            ThermalManager::new().expect("Failed to initialize thermal manager"),
        ));

        // Build profile manager merging built-ins + custom profiles from config
        let custom = config_manager.lock().unwrap().get_config().custom_profiles.clone();
        let profile_manager = Arc::new(Mutex::new({
            let mut pm = ProfileManager::new();
            for p in custom { pm.add_profile(p); }
            pm
        }));

        let cpu_info = cpu_manager.lock().unwrap().get_cpu_info().unwrap_or_else(|_| CpuInfo {
            model: "Unknown".into(), vendor: "Unknown".into(), core_count: 0,
            driver: CpuDriver::Unknown, min_freq: 0, max_freq: 0,
            available_governors: vec![], scaling_available_frequencies: vec![],
        });

        let core_count = cpu_info.core_count;

        let window = ApplicationWindow::builder()
            .application(app)
            .title("CPU Power Manager")
            .default_width(1100)
            .default_height(800)
            .build();

        let freq_label    = Label::new(Some("-- MHz"));
        let usage_label   = Label::new(Some("-- %"));
        let temp_label    = Label::new(Some("--°C"));
        let governor_label = Label::new(Some("--"));
        let turbo_label   = Label::new(Some("--"));
        let power_label   = Label::new(Some("-- W"));
        let battery_label = Label::new(Some("--"));
        let fan_label     = Label::new(Some("-- RPM"));
        let active_profile_label = Label::new(Some("--"));
        let per_core_box  = Box::new(Orientation::Vertical, 4);

        let cpu_usage_area    = gtk4::DrawingArea::new();
        cpu_usage_area.set_content_width(600);
        cpu_usage_area.set_content_height(160);
        let cpu_usage_history = Arc::new(Mutex::new(vec![0.0f32; 60]));
        let cpu_usage_tracker = Arc::new(Mutex::new(CpuUsageTracker::new()));
        let per_core_tracker  = Arc::new(Mutex::new(PerCoreCpuUsageTracker::new(core_count)));
        let rapl_tracker      = Arc::new(Mutex::new(RaplTracker::new()));
        let last_ac_status    = Arc::new(Mutex::new(None::<bool>));

        let app_window = Self {
            window, cpu_manager, thermal_manager, profile_manager, config_manager,
            cpu_info, freq_label, usage_label, temp_label, governor_label, turbo_label,
            power_label, battery_label, fan_label, active_profile_label, per_core_box,
            cpu_usage_area, cpu_usage_history, cpu_usage_tracker, per_core_tracker,
            rapl_tracker, last_ac_status,
        };

        app_window.setup_ui();
        app_window
    }

    fn setup_ui(&self) {
        // ── Header bar ───────────────────────────────────────────────────────────
        let header = HeaderBar::new();
        header.set_show_title_buttons(false);

        let traffic_box = Box::new(Orientation::Horizontal, 7);
        traffic_box.set_margin_start(14);
        traffic_box.set_valign(gtk4::Align::Center);

        // Close — Dracula red #ff5555
        let close_dot = traffic_dot(1.0, 0.333, 0.333);
        let wc = self.window.clone();
        let gc = gtk4::GestureClick::new();
        gc.connect_released(move |_, _, _, _| { wc.close(); });
        close_dot.add_controller(gc);

        // Minimize — Dracula yellow #f1fa8c
        let min_dot = traffic_dot(0.945, 0.980, 0.549);
        let wc = self.window.clone();
        let gc = gtk4::GestureClick::new();
        gc.connect_released(move |_, _, _, _| { wc.minimize(); });
        min_dot.add_controller(gc);

        // Maximize — Dracula green #50fa7b
        let max_dot = traffic_dot(0.314, 0.980, 0.482);
        let wc = self.window.clone();
        let gc = gtk4::GestureClick::new();
        gc.connect_released(move |_, _, _, _| {
            if wc.is_maximized() { wc.unmaximize(); } else { wc.maximize(); }
        });
        max_dot.add_controller(gc);

        traffic_box.append(&close_dot);
        traffic_box.append(&min_dot);
        traffic_box.append(&max_dot);
        header.pack_start(&traffic_box);

        let title_box = Box::new(Orientation::Vertical, 0);
        let title = Label::new(Some("CPU Power Manager"));
        title.add_css_class("header-title");
        let subtitle = Label::new(Some("Advanced CPU Control"));
        subtitle.add_css_class("header-subtitle");
        title_box.append(&title);
        title_box.append(&subtitle);
        header.set_title_widget(Some(&title_box));

        // About button
        let about_btn = Button::with_label("ℹ");
        about_btn.set_tooltip_text(Some("About"));
        about_btn.add_css_class("traffic-btn");
        let wc = self.window.clone();
        about_btn.connect_clicked(move |_| {
            let dlg = AboutDialog::new();
            dlg.set_program_name(Some("CPU Power Manager"));
            dlg.set_version(Some(env!("CARGO_PKG_VERSION")));
            dlg.set_comments(Some("Advanced CPU power management for Linux.\nSupports Intel & AMD CPUs with per-core control, profiles, and real-time monitoring."));
            dlg.set_website(Some("https://www.jegly.xyz"));
            dlg.set_website_label("www.jegly.xyz");
            dlg.set_authors(&["JEGLY"]);
            dlg.set_license_type(License::Gpl30);
            // Load logo from embedded SVG bytes so it works without installation
            let svg_bytes = glib::Bytes::from_static(include_bytes!("../assets/icon.svg"));
            if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&svg_bytes) {
                dlg.set_logo(Some(&texture));
            }
            dlg.set_transient_for(Some(&wc));
            dlg.present();
        });
        header.pack_end(&about_btn);

        // Profile quick-switcher
        let profile_menu_btn = MenuButton::new();
        profile_menu_btn.set_label("⚡ Profiles");
        let profile_popover = Popover::new();
        let pop_box = Box::new(Orientation::Vertical, 6);
        pop_box.set_margin_top(10); pop_box.set_margin_bottom(10);
        pop_box.set_margin_start(12); pop_box.set_margin_end(12);
        let pop_title = Label::new(Some("Switch Profile"));
        pop_title.add_css_class("title");
        pop_box.append(&pop_title);
        pop_box.append(&Separator::new(Orientation::Horizontal));
        {
            let pm = self.profile_manager.lock().unwrap();
            for profile in pm.get_profiles() {
                let btn = Button::with_label(&profile.name);
                btn.set_tooltip_text(Some(&profile.description));
                let cpu_c = self.cpu_manager.clone();
                let pop_c = profile_popover.clone();
                let lbl_c = self.active_profile_label.clone();
                let cfg_c = self.config_manager.clone();
                let p     = profile.clone();
                btn.connect_clicked(move |_| {
                    let result = { let cpu = cpu_c.lock().unwrap(); p.apply(&cpu) };
                    if result.is_ok() {
                        lbl_c.set_text(&p.name);
                        let mut cfg = cfg_c.lock().unwrap();
                        cfg.get_config_mut().general.last_profile = p.name.clone();
                        let _ = cfg.save();
                    }
                    pop_c.popdown();
                });
                pop_box.append(&btn);
            }
        }
        profile_popover.set_child(Some(&pop_box));
        profile_menu_btn.set_popover(Some(&profile_popover));
        header.pack_end(&profile_menu_btn);
        self.window.set_titlebar(Some(&header));

        // Minimize-to-tray on close
        let cfg_close = self.config_manager.clone();
        self.window.connect_close_request(move |win| {
            if cfg_close.lock().unwrap().get_config().general.minimize_to_tray {
                win.hide();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });

        // ── Tabs ─────────────────────────────────────────────────────────────────
        let notebook = Notebook::new();
        notebook.set_vexpand(true);

        // Tab 1: Monitor
        let mon_scroll = ScrolledWindow::new();
        mon_scroll.set_vexpand(true);
        let mon_box = Box::new(Orientation::Vertical, 12);
        mon_box.set_margin_top(12); mon_box.set_margin_bottom(12);
        mon_box.set_margin_start(12); mon_box.set_margin_end(12);
        mon_box.append(&self.create_dashboard());
        mon_box.append(&self.create_graph());
        mon_box.append(&self.create_per_core_section());
        mon_scroll.set_child(Some(&mon_box));
        notebook.append_page(&mon_scroll, Some(&Label::new(Some("📊 Monitor"))));

        // Tab 2: Control
        let ctrl_scroll = ScrolledWindow::new();
        ctrl_scroll.set_vexpand(true);
        let ctrl_box = Box::new(Orientation::Vertical, 12);
        ctrl_box.set_margin_top(12); ctrl_box.set_margin_bottom(12);
        ctrl_box.set_margin_start(12); ctrl_box.set_margin_end(12);
        ctrl_box.append(&self.create_profile_buttons());
        ctrl_box.append(&self.create_freq_sliders());
        ctrl_box.append(&self.create_advanced_controls());
        ctrl_box.append(&self.create_ac_battery_section());
        ctrl_scroll.set_child(Some(&ctrl_box));
        notebook.append_page(&ctrl_scroll, Some(&Label::new(Some("🎛 Control"))));

        // Tab 3: Settings
        let set_scroll = ScrolledWindow::new();
        set_scroll.set_vexpand(true);
        let set_box = Box::new(Orientation::Vertical, 12);
        set_box.set_margin_top(12); set_box.set_margin_bottom(12);
        set_box.set_margin_start(12); set_box.set_margin_end(12);
        set_box.append(&self.create_system_settings());
        set_box.append(&self.create_app_settings());
        set_box.append(&self.create_custom_profiles_section());
        set_box.append(&self.create_system_info());
        set_scroll.set_child(Some(&set_box));
        notebook.append_page(&set_scroll, Some(&Label::new(Some("⚙ Settings"))));

        self.window.set_child(Some(&notebook));
        self.setup_updates();
    }

    // ── Dashboard (2×4 grid) ─────────────────────────────────────────────────────

    fn create_dashboard(&self) -> Frame {
        let frame = Frame::new(Some("System Overview"));
        frame.add_css_class("card");

        let grid = Grid::new();
        grid.set_row_spacing(16);
        grid.set_column_spacing(12);
        grid.set_column_homogeneous(true);
        grid.set_margin_top(12); grid.set_margin_bottom(12);
        grid.set_margin_start(12); grid.set_margin_end(12);

        let cards: Vec<(&str, &Label, Option<&str>, Option<&str>)> = vec![
            ("Avg Frequency", &self.freq_label, Some("freq-value"), None),
            ("CPU Usage",     &self.usage_label, Some("usage-value"), None),
            ("Temperature",   &self.temp_label, None, None),
            ("Governor",      &self.governor_label, Some("value"), None),
            ("Turbo Boost",   &self.turbo_label, None, None),
            ("Power Draw",    &self.power_label, Some("value"), None),
            ("Battery",       &self.battery_label, None, None),
            ("Fan Speed",     &self.fan_label, Some("value"), None),
        ];

        // Only place the first 7 cards (skip index 7 / Fan Speed — it lives in the CPU card below)
        for (i, (title, lbl, css, _)) in cards.iter().take(7).enumerate() {
            let col = (i % 4) as i32;
            let row = (i / 4) as i32;
            let card = Box::new(Orientation::Vertical, 4);
            card.set_hexpand(true);
            let t = Label::new(Some(title));
            t.add_css_class("subtitle");
            t.set_halign(gtk4::Align::Start);
            card.append(&t);
            if let Some(c) = css { lbl.add_css_class(c); }
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
            card.append(*lbl);
            grid.attach(&card, col, row, 1, 1);
        }

        // CPU Info card at (3,1) — previously overlapped with Fan Speed card
        let cpu_card = Box::new(Orientation::Vertical, 4);
        cpu_card.set_hexpand(true);
        let ct = Label::new(Some("CPU"));
        ct.add_css_class("subtitle");
        ct.set_halign(gtk4::Align::Start);
        let model_lbl = Label::new(Some(&self.cpu_info.model));
        model_lbl.set_halign(gtk4::Align::Start);
        model_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let detail_lbl = Label::new(Some(&format!(
            "{} cores · {} – {} MHz",
            self.cpu_info.core_count,
            self.cpu_info.min_freq, self.cpu_info.max_freq
        )));
        detail_lbl.add_css_class("subtitle");
        detail_lbl.set_halign(gtk4::Align::Start);
        detail_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let fan_title = Label::new(Some("Fan Speed"));
        fan_title.add_css_class("subtitle");
        fan_title.set_halign(gtk4::Align::Start);
        self.fan_label.add_css_class("value");
        self.fan_label.set_halign(gtk4::Align::Start);
        let profile_title = Label::new(Some("Active Profile"));
        profile_title.add_css_class("subtitle");
        profile_title.set_halign(gtk4::Align::Start);
        self.active_profile_label.add_css_class("value");
        self.active_profile_label.set_halign(gtk4::Align::Start);
        cpu_card.append(&ct);
        cpu_card.append(&model_lbl);
        cpu_card.append(&detail_lbl);
        cpu_card.append(&fan_title);
        cpu_card.append(&self.fan_label);
        cpu_card.append(&profile_title);
        cpu_card.append(&self.active_profile_label);
        grid.attach(&cpu_card, 3, 1, 1, 1);

        frame.set_child(Some(&grid));
        frame
    }

    // ── Usage graph ───────────────────────────────────────────────────────────────

    fn create_graph(&self) -> Frame {
        let frame = Frame::new(Some("CPU Usage History (60s)"));
        frame.add_css_class("card");

        let vbox = Box::new(Orientation::Vertical, 8);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let history = self.cpu_usage_history.clone();
        self.cpu_usage_area.set_draw_func(move |_area, cr, width, height| {
            let h = history.lock().unwrap();
            // Dracula bg_secondary #1e1f29
            cr.set_source_rgb(0.118, 0.122, 0.161);
            let _ = cr.paint();
            for i in 0..=4 {
                let y = (i as f64 / 4.0) * height as f64;
                // Dracula bg_elevated #44475a
                cr.set_source_rgba(0.267, 0.278, 0.353, 0.7);
                cr.set_line_width(1.0);
                let _ = cr.move_to(0.0, y); let _ = cr.line_to(width as f64, y);
                let _ = cr.stroke();
                // Dracula comment #6272a4
                cr.set_source_rgba(0.384, 0.447, 0.643, 0.9);
                let _ = cr.move_to(4.0, y - 2.0);
                cr.set_font_size(10.0);
                let _ = cr.show_text(&format!("{}%", 100 - i * 25));
            }
            if h.len() > 1 {
                let sp = width as f64 / (h.len() - 1) as f64;
                // Dracula purple #bd93f9 fill
                cr.set_source_rgba(0.741, 0.576, 0.976, 0.2);
                let _ = cr.move_to(0.0, height as f64);
                let _ = cr.line_to(0.0, height as f64 - h[0] as f64 / 100.0 * height as f64);
                for (i, &u) in h.iter().enumerate().skip(1) {
                    let _ = cr.line_to(i as f64 * sp, height as f64 - u as f64 / 100.0 * height as f64);
                }
                let _ = cr.line_to(width as f64, height as f64);
                let _ = cr.close_path(); let _ = cr.fill();
                // Dracula purple #bd93f9 line
                cr.set_source_rgb(0.741, 0.576, 0.976);
                cr.set_line_width(2.0);
                let _ = cr.move_to(0.0, height as f64 - h[0] as f64 / 100.0 * height as f64);
                for (i, &u) in h.iter().enumerate().skip(1) {
                    let _ = cr.line_to(i as f64 * sp, height as f64 - u as f64 / 100.0 * height as f64);
                }
                let _ = cr.stroke();
                if let Some(&last) = h.last() {
                    let x = (h.len() - 1) as f64 * sp;
                    let y = height as f64 - last as f64 / 100.0 * height as f64;
                    // Dracula accent_hover #caa9fa dot
                    cr.set_source_rgb(0.792, 0.663, 0.980);
                    let _ = cr.arc(x, y, 3.5, 0.0, std::f64::consts::TAU);
                    let _ = cr.fill();
                }
            }
        });

        vbox.append(&self.cpu_usage_area);
        let lbl = Label::new(Some("← 60 seconds →"));
        lbl.add_css_class("subtitle");
        lbl.set_halign(gtk4::Align::Center);
        vbox.append(&lbl);
        frame.set_child(Some(&vbox));
        frame
    }

    // ── Per-core panel ────────────────────────────────────────────────────────────

    fn create_per_core_section(&self) -> Frame {
        let frame = Frame::new(Some("Per-Core Status"));
        frame.add_css_class("card");
        self.per_core_box.set_margin_top(8);
        self.per_core_box.set_margin_bottom(8);
        self.per_core_box.set_margin_start(12);
        self.per_core_box.set_margin_end(12);
        frame.set_child(Some(&self.per_core_box));
        frame
    }

    fn rebuild_per_core_panel(&self, usages: &[f32]) {
        while let Some(child) = self.per_core_box.first_child() {
            self.per_core_box.remove(&child);
        }
        let cpu = self.cpu_manager.lock().unwrap();
        let core_temps = HwmonReader::get_per_core_temps();
        let grid = Grid::new();
        grid.set_row_spacing(4);
        grid.set_column_spacing(8);

        if let Ok(statuses) = cpu.get_all_core_status() {
            for (i, status) in statuses.iter().enumerate() {
                let is_p = cpu.is_p_core(status.core_id);
                let core_type = if is_p { "P" } else { "E" };
                let row_box = Box::new(Orientation::Horizontal, 8);
                row_box.add_css_class("freq-display");
                row_box.set_hexpand(true);

                // Core label with P/E tag
                let core_lbl = Label::new(Some(&format!("Core {:2} [{}]", status.core_id, core_type)));
                core_lbl.set_halign(gtk4::Align::Start);
                core_lbl.set_width_chars(14);

                // Frequency
                let freq_lbl = Label::new(Some(&format!("{:4} MHz", status.current_freq)));
                freq_lbl.add_css_class("value");
                freq_lbl.set_width_chars(9);

                // Governor
                let gov_lbl = Label::new(Some(&format!("[{}]", status.governor)));
                gov_lbl.add_css_class("subtitle");
                gov_lbl.set_width_chars(12);

                // Usage bar
                let usage_pct = usages.get(i).copied().unwrap_or(0.0);
                let bar = LevelBar::new();
                bar.set_min_value(0.0);
                bar.set_max_value(100.0);
                bar.set_value(usage_pct as f64);
                bar.set_hexpand(true);
                bar.set_valign(gtk4::Align::Center);

                let pct_lbl = Label::new(Some(&format!("{:3.0}%", usage_pct)));
                pct_lbl.set_width_chars(5);

                // Per-core temp if available
                let temp_lbl = if let Some(&(_, temp)) = core_temps.iter().find(|(id, _)| *id == status.core_id) {
                    let l = Label::new(Some(&format!("{:.0}°C", temp)));
                    l.add_css_class("subtitle");
                    l.set_width_chars(6);
                    Some(l)
                } else { None };

                // Online toggle (core 0 can't be taken offline)
                let online_sw = Switch::new();
                online_sw.set_active(status.online);
                online_sw.set_valign(gtk4::Align::Center);
                online_sw.set_sensitive(status.core_id != 0);
                let cpu_c = self.cpu_manager.clone();
                let core_id = status.core_id;
                online_sw.connect_state_set(move |_, state| {
                    let cpu = cpu_c.lock().unwrap();
                    if let Err(e) = cpu.set_core_online(core_id, state) {
                        log::warn!("Core {} online toggle failed: {}", core_id, e);
                    }
                    glib::Propagation::Proceed
                });

                row_box.append(&core_lbl);
                row_box.append(&freq_lbl);
                row_box.append(&gov_lbl);
                row_box.append(&bar);
                row_box.append(&pct_lbl);
                if let Some(t) = &temp_lbl { row_box.append(t); }
                row_box.append(&online_sw);

                let col = (i % 2) as i32;
                let row = (i / 2) as i32;
                grid.attach(&row_box, col, row, 1, 1);
            }
        }
        self.per_core_box.append(&grid);
    }

    // ── Profile buttons ───────────────────────────────────────────────────────────

    fn create_profile_buttons(&self) -> Frame {
        let frame = Frame::new(Some("Quick Profiles"));
        frame.add_css_class("card");
        let section = Box::new(Orientation::Vertical, 12);
        section.set_margin_top(12); section.set_margin_bottom(12);
        section.set_margin_start(12); section.set_margin_end(12);
        let profiles_box = Box::new(Orientation::Horizontal, 8);
        profiles_box.set_halign(gtk4::Align::Center);

        let pm = self.profile_manager.lock().unwrap();
        for profile in pm.get_profiles() {
            let btn = Button::with_label(&profile.name);
            btn.set_tooltip_text(Some(&profile.description));
            let cpu_c   = self.cpu_manager.clone();
            let lbl_c   = self.active_profile_label.clone();
            let cfg_c   = self.config_manager.clone();
            let p       = profile.clone();
            btn.connect_clicked(move |b| {
                let result = { let cpu = cpu_c.lock().unwrap(); p.apply(&cpu) };
                match result {
                    Ok(_) => {
                        lbl_c.set_text(&p.name);
                        b.set_label(&format!("✓ {}", p.name));
                        let mut cfg = cfg_c.lock().unwrap();
                        cfg.get_config_mut().general.last_profile = p.name.clone();
                        let _ = cfg.save();
                    }
                    Err(e) => {
                        log::error!("Profile apply failed: {}", e);
                        b.set_label(&format!("✗ {}", p.name));
                    }
                }
                let bc = b.clone();
                let name = p.name.clone();
                glib::timeout_add_seconds_local(2, move || { bc.set_label(&name); glib::ControlFlow::Break });
            });
            profiles_box.append(&btn);
        }
        section.append(&profiles_box);
        frame.set_child(Some(&section));
        frame
    }

    // ── Frequency sliders ─────────────────────────────────────────────────────────

    fn create_freq_sliders(&self) -> Frame {
        let frame = Frame::new(Some("Frequency Limits"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 12);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let cpu = self.cpu_manager.lock().unwrap();
        let hw_min = cpu.get_hardware_min_freq(0).unwrap_or(400) as f64;
        let hw_max = cpu.get_hardware_max_freq(0).unwrap_or(4000) as f64;
        let cur_min = cpu.get_scaling_min_freq(0).unwrap_or(hw_min as u32) as f64;
        let cur_max = cpu.get_scaling_max_freq(0).unwrap_or(hw_max as u32) as f64;
        drop(cpu);

        // Min freq slider
        let min_row = Box::new(Orientation::Horizontal, 12);
        min_row.add_css_class("settings-row");
        let min_lbl_box = Box::new(Orientation::Vertical, 2);
        min_lbl_box.set_hexpand(true);
        let min_title = Label::new(Some("Minimum Frequency"));
        min_title.set_halign(gtk4::Align::Start);
        let min_val_lbl = Label::new(Some(&format!("{} MHz", cur_min as u32)));
        min_val_lbl.add_css_class("value");
        min_val_lbl.set_halign(gtk4::Align::Start);
        min_lbl_box.append(&min_title);
        min_lbl_box.append(&min_val_lbl);
        let min_slider = Scale::with_range(Orientation::Horizontal, hw_min, hw_max, 100.0);
        min_slider.set_value(cur_min);
        min_slider.set_hexpand(true);
        min_slider.set_width_request(300);
        let lbl_c = min_val_lbl.clone();
        min_slider.connect_value_changed(move |s| {
            lbl_c.set_text(&format!("{} MHz", s.value() as u32));
        });
        min_row.append(&min_lbl_box);
        min_row.append(&min_slider);

        // Max freq slider
        let max_row = Box::new(Orientation::Horizontal, 12);
        max_row.add_css_class("settings-row");
        let max_lbl_box = Box::new(Orientation::Vertical, 2);
        max_lbl_box.set_hexpand(true);
        let max_title = Label::new(Some("Maximum Frequency"));
        max_title.set_halign(gtk4::Align::Start);
        let max_val_lbl = Label::new(Some(&format!("{} MHz", cur_max as u32)));
        max_val_lbl.add_css_class("value");
        max_val_lbl.set_halign(gtk4::Align::Start);
        max_lbl_box.append(&max_title);
        max_lbl_box.append(&max_val_lbl);
        let max_slider = Scale::with_range(Orientation::Horizontal, hw_min, hw_max, 100.0);
        max_slider.set_value(cur_max);
        max_slider.set_hexpand(true);
        max_slider.set_width_request(300);
        let lbl_c = max_val_lbl.clone();
        max_slider.connect_value_changed(move |s| {
            lbl_c.set_text(&format!("{} MHz", s.value() as u32));
        });
        max_row.append(&max_lbl_box);
        max_row.append(&max_slider);

        // Apply button
        let apply_btn = Button::with_label("Apply Limits");
        apply_btn.add_css_class("suggested-action");
        apply_btn.set_halign(gtk4::Align::Center);
        let cpu_c = self.cpu_manager.clone();
        let min_s = min_slider.clone();
        let max_s = max_slider.clone();
        apply_btn.connect_clicked(move |btn| {
            let min_mhz = min_s.value() as u32;
            let max_mhz = max_s.value() as u32;
            let cpu = cpu_c.lock().unwrap();
            let mut ok = true;
            for core in 0..cpu.core_count() {
                if cpu.set_scaling_min_freq(core, min_mhz).is_err() { ok = false; }
                if cpu.set_scaling_max_freq(core, max_mhz).is_err() { ok = false; }
            }
            btn.set_label(if ok { "✓ Applied" } else { "⚠ Partial (E-cores skipped)" });
            let bc = btn.clone();
            glib::timeout_add_seconds_local(2, move || { bc.set_label("Apply Limits"); glib::ControlFlow::Break });
        });

        let note = Label::new(Some("Drag sliders then click Apply. Changes apply to all P-cores; E-cores use shared policy."));
        note.add_css_class("subtitle");
        note.set_wrap(true);

        vbox.append(&min_row);
        vbox.append(&max_row);
        vbox.append(&apply_btn);
        vbox.append(&note);
        frame.set_child(Some(&vbox));
        frame
    }

    // ── Advanced controls ─────────────────────────────────────────────────────────

    fn create_advanced_controls(&self) -> Frame {
        let frame = Frame::new(Some("Advanced Controls"));
        frame.add_css_class("card");
        let grid = Grid::new();
        grid.set_row_spacing(12); grid.set_column_spacing(12);
        grid.set_margin_top(12); grid.set_margin_bottom(12);
        grid.set_margin_start(12); grid.set_margin_end(12);

        let cpu = self.cpu_manager.lock().unwrap();

        // Governor
        let gov_lbl = Label::new(Some("Governor:"));
        gov_lbl.set_halign(gtk4::Align::End);
        grid.attach(&gov_lbl, 0, 0, 1, 1);
        let governors = cpu.get_available_governors(0).unwrap_or_default();
        let sl = StringList::new(&governors.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let gov_dd = DropDown::new(Some(sl), None::<gtk4::Expression>);
        if let Ok(cur) = cpu.get_governor(0) {
            if let Some(pos) = governors.iter().position(|g| g == &cur) {
                gov_dd.set_selected(pos as u32);
            }
        }
        let govs_c = governors.clone();
        let cpu_c  = self.cpu_manager.clone();
        let lbl_c  = self.active_profile_label.clone();
        gov_dd.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if idx < govs_c.len() {
                let cpu = cpu_c.lock().unwrap();
                match cpu.set_governor_all(&govs_c[idx]) {
                    Ok(_) => lbl_c.set_text(&format!("Custom ({})", govs_c[idx])),
                    Err(e) => log::error!("Governor set failed: {}", e),
                }
            }
        });
        grid.attach(&gov_dd, 1, 0, 1, 1);

        // Turbo
        let turbo_lbl_row = Label::new(Some("Turbo Boost:"));
        turbo_lbl_row.set_halign(gtk4::Align::End);
        grid.attach(&turbo_lbl_row, 0, 1, 1, 1);
        let turbo_box = Box::new(Orientation::Horizontal, 8);
        let turbo_sw  = gtk4::Switch::new();
        turbo_sw.add_css_class("turbo-switch");
        let turbo_on  = cpu.is_turbo_enabled().unwrap_or(false);
        turbo_sw.set_active(turbo_on);
        let status_lbl = Label::new(Some(if turbo_on { "ON" } else { "OFF" }));
        status_lbl.add_css_class("turbo-status");
        status_lbl.add_css_class(if turbo_on { "turbo-on" } else { "turbo-off" });
        let cpu_c2 = self.cpu_manager.clone();
        let sl_c   = status_lbl.clone();
        turbo_sw.connect_state_set(move |_, state| {
            let _ = cpu_c2.lock().unwrap().set_turbo(state);
            sl_c.set_text(if state { "ON" } else { "OFF" });
            sl_c.remove_css_class("turbo-on"); sl_c.remove_css_class("turbo-off");
            sl_c.add_css_class(if state { "turbo-on" } else { "turbo-off" });
            glib::Propagation::Proceed
        });
        turbo_box.append(&turbo_sw);
        turbo_box.append(&status_lbl);
        grid.attach(&turbo_box, 1, 1, 1, 1);

        // Max freq button
        let max_btn = Button::with_label("⚡ Set All Cores to Hardware Maximum");
        max_btn.add_css_class("suggested-action");
        max_btn.set_margin_top(4);
        let cpu_c3 = self.cpu_manager.clone();
        let lbl_c3 = self.active_profile_label.clone();
        max_btn.connect_clicked(move |btn| {
            let result: anyhow::Result<u32> = (|| {
                let cpu = cpu_c3.lock().unwrap();
                let max = cpu.get_hardware_max_freq(0)?;
                let min = cpu.get_hardware_min_freq(0)?;
                for c in 0..cpu.core_count() {
                    let _ = cpu.set_scaling_min_freq(c, min);
                    let _ = cpu.set_scaling_max_freq(c, max);
                }
                cpu.set_governor_all("performance")?;
                let _ = cpu.set_turbo(true);
                Ok(max)
            })();
            match result {
                Ok(f) => { lbl_c3.set_text("Max Freq"); btn.set_label(&format!("✓ {} MHz", f)); }
                Err(e) => { log::error!("{}", e); btn.set_label("✗ Failed — root required?"); }
            }
            let bc = btn.clone();
            glib::timeout_add_seconds_local(3, move || { bc.set_label("⚡ Set All Cores to Hardware Maximum"); glib::ControlFlow::Break });
        });
        grid.attach(&max_btn, 0, 2, 2, 1);

        frame.set_child(Some(&grid));
        frame
    }

    // ── AC / Battery auto-switch ──────────────────────────────────────────────────

    fn create_ac_battery_section(&self) -> Frame {
        let frame = Frame::new(Some("AC / Battery Auto-Switch"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 10);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let cfg = self.config_manager.lock().unwrap().get_config().clone();

        // Enable toggle
        let enable_row = Box::new(Orientation::Horizontal, 12);
        enable_row.add_css_class("settings-row");
        let en_lbl_box = Box::new(Orientation::Vertical, 2);
        en_lbl_box.set_hexpand(true);
        let en_title = Label::new(Some("Auto-Switch Profiles"));
        en_title.set_halign(gtk4::Align::Start);
        let en_sub = Label::new(Some("Apply AC or battery profile when power source changes"));
        en_sub.add_css_class("subtitle"); en_sub.set_halign(gtk4::Align::Start);
        en_lbl_box.append(&en_title); en_lbl_box.append(&en_sub);
        let enable_sw = gtk4::Switch::new();
        enable_sw.set_active(cfg.auto_tune.enabled);
        enable_sw.set_valign(gtk4::Align::Center);
        let cfg_c = self.config_manager.clone();
        enable_sw.connect_state_set(move |_, state| {
            let mut c = cfg_c.lock().unwrap();
            c.get_config_mut().auto_tune.enabled = state;
            let _ = c.save();
            glib::Propagation::Proceed
        });
        enable_row.append(&en_lbl_box);
        enable_row.append(&enable_sw);

        // Profile names from profile manager
        let profile_names: Vec<String> = self.profile_manager.lock().unwrap()
            .get_profiles().iter().map(|p| p.name.clone()).collect();

        // AC profile
        let ac_row = self.make_profile_row(
            "AC Power Profile",
            "Applied when plugged in",
            &profile_names,
            &cfg.auto_tune.ac_profile,
            {
                let c = self.config_manager.clone();
                let names = profile_names.clone();
                move |idx| {
                    let mut cfg = c.lock().unwrap();
                    cfg.get_config_mut().auto_tune.ac_profile = names[idx].clone();
                    let _ = cfg.save();
                }
            },
        );

        // Battery profile
        let bat_row = self.make_profile_row(
            "Battery Profile",
            "Applied when on battery",
            &profile_names,
            &cfg.auto_tune.battery_profile,
            {
                let c = self.config_manager.clone();
                let names = profile_names.clone();
                move |idx| {
                    let mut cfg = c.lock().unwrap();
                    cfg.get_config_mut().auto_tune.battery_profile = names[idx].clone();
                    let _ = cfg.save();
                }
            },
        );

        vbox.append(&enable_row);
        vbox.append(&ac_row);
        vbox.append(&bat_row);
        frame.set_child(Some(&vbox));
        frame
    }

    fn make_profile_row(
        &self,
        title: &str,
        subtitle: &str,
        names: &[String],
        current: &str,
        on_change: impl Fn(usize) + 'static,
    ) -> Box {
        let row = Box::new(Orientation::Horizontal, 12);
        row.add_css_class("settings-row");
        let lbl_box = Box::new(Orientation::Vertical, 2);
        lbl_box.set_hexpand(true);
        let t = Label::new(Some(title));
        t.set_halign(gtk4::Align::Start);
        let s = Label::new(Some(subtitle));
        s.add_css_class("subtitle"); s.set_halign(gtk4::Align::Start);
        lbl_box.append(&t); lbl_box.append(&s);
        let sl = StringList::new(&names.iter().map(|n| n.as_str()).collect::<Vec<_>>());
        let dd = DropDown::new(Some(sl), None::<gtk4::Expression>);
        dd.set_valign(gtk4::Align::Center);
        let names_c = names.to_vec();
        if let Some(pos) = names_c.iter().position(|n| n == current) {
            dd.set_selected(pos as u32);
        }
        dd.connect_selected_notify(move |d| {
            let idx = d.selected() as usize;
            if idx < names_c.len() { on_change(idx); }
        });
        row.append(&lbl_box);
        row.append(&dd);
        row
    }

    // ── System settings tab ───────────────────────────────────────────────────────

    fn create_system_settings(&self) -> Frame {
        let frame = Frame::new(Some("System Settings"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 10);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        // Reset frequency limits
        let reset_row = Box::new(Orientation::Horizontal, 12);
        reset_row.add_css_class("settings-row");
        let r_lbl = Box::new(Orientation::Vertical, 2); r_lbl.set_hexpand(true);
        let rt = Label::new(Some("Reset Frequency Limits")); rt.set_halign(gtk4::Align::Start);
        let rs = Label::new(Some("Restore hardware min/max for all cores"));
        rs.add_css_class("subtitle"); rs.set_halign(gtk4::Align::Start);
        r_lbl.append(&rt); r_lbl.append(&rs);
        let reset_btn = Button::with_label("Reset");
        reset_btn.set_valign(gtk4::Align::Center);
        let cpu_c = self.cpu_manager.clone();
        reset_btn.connect_clicked(move |btn| {
            let result: anyhow::Result<()> = (|| {
                let cpu = cpu_c.lock().unwrap();
                let min = cpu.get_hardware_min_freq(0)?;
                let max = cpu.get_hardware_max_freq(0)?;
                for c in 0..cpu.core_count() {
                    let _ = cpu.set_scaling_min_freq(c, min);
                    let _ = cpu.set_scaling_max_freq(c, max);
                }
                Ok(())
            })();
            btn.set_label(if result.is_ok() { "✓ Done" } else { "✗ Failed" });
            let bc = btn.clone();
            glib::timeout_add_seconds_local(2, move || { bc.set_label("Reset"); glib::ControlFlow::Break });
        });
        reset_row.append(&r_lbl); reset_row.append(&reset_btn);
        vbox.append(&reset_row);

        // Turbo
        let turbo_row = Box::new(Orientation::Horizontal, 12);
        turbo_row.add_css_class("settings-row");
        let t_lbl = Box::new(Orientation::Vertical, 2); t_lbl.set_hexpand(true);
        let tt = Label::new(Some("Turbo / Boost")); tt.set_halign(gtk4::Align::Start);
        let ts = Label::new(Some("Allow CPU to exceed base clock"));
        ts.add_css_class("subtitle"); ts.set_halign(gtk4::Align::Start);
        t_lbl.append(&tt); t_lbl.append(&ts);
        let turbo_sw2 = gtk4::Switch::new();
        turbo_sw2.set_valign(gtk4::Align::Center);
        turbo_sw2.set_active(self.cpu_manager.lock().unwrap().is_turbo_enabled().unwrap_or(false));
        let cpu_c2 = self.cpu_manager.clone();
        turbo_sw2.connect_state_set(move |_, state| {
            let _ = cpu_c2.lock().unwrap().set_turbo(state);
            glib::Propagation::Proceed
        });
        turbo_row.append(&t_lbl); turbo_row.append(&turbo_sw2);
        vbox.append(&turbo_row);

        // Governor
        let gov_row = Box::new(Orientation::Horizontal, 12);
        gov_row.add_css_class("settings-row");
        let g_lbl = Box::new(Orientation::Vertical, 2); g_lbl.set_hexpand(true);
        let gt = Label::new(Some("Global Governor")); gt.set_halign(gtk4::Align::Start);
        let gs = Label::new(Some("Apply same governor to all cores"));
        gs.add_css_class("subtitle"); gs.set_halign(gtk4::Align::Start);
        g_lbl.append(&gt); g_lbl.append(&gs);
        let govs = self.cpu_manager.lock().unwrap().get_available_governors(0).unwrap_or_default();
        let sl2 = StringList::new(&govs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let gov_dd2 = DropDown::new(Some(sl2), None::<gtk4::Expression>);
        gov_dd2.set_valign(gtk4::Align::Center);
        if let Ok(cur) = self.cpu_manager.lock().unwrap().get_governor(0) {
            if let Some(pos) = govs.iter().position(|g| *g == cur) {
                gov_dd2.set_selected(pos as u32);
            }
        }
        let govs_c2 = govs.clone();
        let cpu_c3  = self.cpu_manager.clone();
        let lbl_c3  = self.active_profile_label.clone();
        gov_dd2.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if idx < govs_c2.len() {
                match cpu_c3.lock().unwrap().set_governor_all(&govs_c2[idx]) {
                    Ok(_) => lbl_c3.set_text(&format!("Custom ({})", govs_c2[idx])),
                    Err(e) => log::error!("Governor: {}", e),
                }
            }
        });
        gov_row.append(&g_lbl); gov_row.append(&gov_dd2);
        vbox.append(&gov_row);

        // EPP (Intel only)
        {
            let cpu = self.cpu_manager.lock().unwrap();
            if cpu.get_epp(0).is_ok() {
                let epp_row = Box::new(Orientation::Horizontal, 12);
                epp_row.add_css_class("settings-row");
                let e_lbl = Box::new(Orientation::Vertical, 2); e_lbl.set_hexpand(true);
                let et = Label::new(Some("Energy Performance Preference")); et.set_halign(gtk4::Align::Start);
                let es = Label::new(Some("Intel EPP — powersave/balance/performance"));
                es.add_css_class("subtitle"); es.set_halign(gtk4::Align::Start);
                e_lbl.append(&et); e_lbl.append(&es);
                let epp_opts = ["performance", "balance_performance", "balance_power", "power"];
                let epp_sl = StringList::new(&epp_opts);
                let epp_dd = DropDown::new(Some(epp_sl), None::<gtk4::Expression>);
                epp_dd.set_valign(gtk4::Align::Center);
                if let Ok(cur_epp) = cpu.get_epp(0) {
                    if let Some(pos) = epp_opts.iter().position(|&e| e == cur_epp.trim()) {
                        epp_dd.set_selected(pos as u32);
                    }
                }
                drop(cpu);
                let cpu_c4 = self.cpu_manager.clone();
                epp_dd.connect_selected_notify(move |dd| {
                    let opts = ["performance", "balance_performance", "balance_power", "power"];
                    let idx = dd.selected() as usize;
                    if idx < opts.len() {
                        if let Err(e) = cpu_c4.lock().unwrap().set_epp(opts[idx]) {
                            log::warn!("EPP: {}", e);
                        }
                    }
                });
                epp_row.append(&e_lbl); epp_row.append(&epp_dd);
                vbox.append(&epp_row);
            }
        }

        frame.set_child(Some(&vbox));
        frame
    }

    // ── App settings tab ──────────────────────────────────────────────────────────

    fn create_app_settings(&self) -> Frame {
        let frame = Frame::new(Some("App Settings"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 10);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let cfg = self.config_manager.lock().unwrap().get_config().clone();

        let settings_rows: Vec<(&str, &str, bool, std::boxed::Box<dyn Fn(bool)>)> = vec![
            (
                "Launch at Login",
                "Start automatically when you log in",
                cfg.general.auto_start,
                std::boxed::Box::new({
                    let c = self.config_manager.clone();
                    move |state| {
                        set_autostart(state);
                        let mut cfg = c.lock().unwrap();
                        cfg.get_config_mut().general.auto_start = state;
                        let _ = cfg.save();
                    }
                }),
            ),
            (
                "Minimize to Tray on Close",
                "Hide window instead of quitting when closed",
                cfg.general.minimize_to_tray,
                std::boxed::Box::new({
                    let c = self.config_manager.clone();
                    move |state| {
                        let mut cfg = c.lock().unwrap();
                        cfg.get_config_mut().general.minimize_to_tray = state;
                        let _ = cfg.save();
                    }
                }),
            ),
            (
                "Auto-Apply Profile on Startup",
                "Restore the last used profile when the app starts",
                cfg.general.auto_apply_on_startup,
                std::boxed::Box::new({
                    let c = self.config_manager.clone();
                    move |state| {
                        let mut cfg = c.lock().unwrap();
                        cfg.get_config_mut().general.auto_apply_on_startup = state;
                        let _ = cfg.save();
                    }
                }),
            ),
        ];

        for (title, sub, active, on_toggle) in settings_rows {
            let row = Box::new(Orientation::Horizontal, 12);
            row.add_css_class("settings-row");
            let lbl_box = Box::new(Orientation::Vertical, 2);
            lbl_box.set_hexpand(true);
            let t = Label::new(Some(title)); t.set_halign(gtk4::Align::Start);
            let s = Label::new(Some(sub)); s.add_css_class("subtitle"); s.set_halign(gtk4::Align::Start);
            lbl_box.append(&t); lbl_box.append(&s);
            let sw = gtk4::Switch::new();
            sw.set_active(active);
            sw.set_valign(gtk4::Align::Center);
            sw.connect_state_set(move |_, state| {
                on_toggle(state);
                glib::Propagation::Proceed
            });
            row.append(&lbl_box);
            row.append(&sw);
            vbox.append(&row);
        }

        // Display size
        let scale_row = Box::new(Orientation::Horizontal, 12);
        scale_row.add_css_class("settings-row");
        let scale_lbl_box = Box::new(Orientation::Vertical, 2);
        scale_lbl_box.set_hexpand(true);
        let scale_t = Label::new(Some("Display Size"));
        scale_t.set_halign(gtk4::Align::Start);
        let scale_s = Label::new(Some("Adjust the UI text and element size"));
        scale_s.add_css_class("subtitle");
        scale_s.set_halign(gtk4::Align::Start);
        scale_lbl_box.append(&scale_t);
        scale_lbl_box.append(&scale_s);
        let scale_options = StringList::new(&["Small", "Normal", "Large", "X-Large"]);
        let scale_dd = DropDown::new(Some(scale_options), gtk4::Expression::NONE);
        scale_dd.set_valign(gtk4::Align::Center);
        let current_scale_idx = match cfg.general.ui_scale.as_str() {
            "small"  => 0u32,
            "large"  => 2,
            "xlarge" => 3,
            _        => 1,
        };
        scale_dd.set_selected(current_scale_idx);
        let cfg_scale = self.config_manager.clone();
        scale_dd.connect_selected_notify(move |dd| {
            let scale = match dd.selected() {
                0 => "small",
                2 => "large",
                3 => "xlarge",
                _ => "normal",
            };
            crate::apply_ui_scale(scale);
            let mut cfg = cfg_scale.lock().unwrap();
            cfg.get_config_mut().general.ui_scale = scale.to_string();
            let _ = cfg.save();
        });
        scale_row.append(&scale_lbl_box);
        scale_row.append(&scale_dd);
        vbox.append(&scale_row);

        // Critical temp notification + threshold
        let notif_row = Box::new(Orientation::Horizontal, 12);
        notif_row.add_css_class("settings-row");
        let n_lbl = Box::new(Orientation::Vertical, 2); n_lbl.set_hexpand(true);
        let nt = Label::new(Some("Critical Temperature Notification"));
        nt.set_halign(gtk4::Align::Start);
        let ns = Label::new(Some("Send desktop notification when CPU exceeds threshold"));
        ns.add_css_class("subtitle"); ns.set_halign(gtk4::Align::Start);
        n_lbl.append(&nt); n_lbl.append(&ns);
        let n_right = Box::new(Orientation::Horizontal, 8);
        let threshold_spin = SpinButton::with_range(60.0, 110.0, 1.0);
        threshold_spin.set_value(cfg.thermal.max_temp_celsius as f64);
        threshold_spin.set_valign(gtk4::Align::Center);
        let deg_lbl = Label::new(Some("°C"));
        let notif_sw = gtk4::Switch::new();
        notif_sw.set_active(cfg.general.critical_temp_notify);
        notif_sw.set_valign(gtk4::Align::Center);
        let cfg_c = self.config_manager.clone();
        let spin_c = threshold_spin.clone();
        notif_sw.connect_state_set(move |_, state| {
            let mut cfg = cfg_c.lock().unwrap();
            cfg.get_config_mut().general.critical_temp_notify = state;
            cfg.get_config_mut().thermal.max_temp_celsius = spin_c.value() as f32;
            let _ = cfg.save();
            glib::Propagation::Proceed
        });
        let cfg_c2 = self.config_manager.clone();
        threshold_spin.connect_value_changed(move |spin| {
            let mut cfg = cfg_c2.lock().unwrap();
            cfg.get_config_mut().thermal.max_temp_celsius = spin.value() as f32;
            let _ = cfg.save();
        });
        n_right.append(&threshold_spin);
        n_right.append(&deg_lbl);
        n_right.append(&notif_sw);
        notif_row.append(&n_lbl);
        notif_row.append(&n_right);
        vbox.append(&notif_row);

        frame.set_child(Some(&vbox));
        frame
    }

    // ── Custom profiles ───────────────────────────────────────────────────────────

    fn create_custom_profiles_section(&self) -> Frame {
        let frame = Frame::new(Some("Custom Profiles"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 10);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let note = Label::new(Some("Custom profiles are saved to ~/.config/cpu-power-manager/config.toml and appear in all profile menus after restart."));
        note.add_css_class("subtitle");
        note.set_wrap(true);
        vbox.append(&note);

        // New profile form
        let form = Box::new(Orientation::Horizontal, 8);
        form.add_css_class("settings-row");

        let name_entry = Entry::new();
        name_entry.set_placeholder_text(Some("Profile name"));
        name_entry.set_hexpand(true);

        let gov_names = self.cpu_manager.lock().unwrap()
            .get_available_governors(0).unwrap_or_default();
        let sl = StringList::new(&gov_names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let gov_dd = DropDown::new(Some(sl), None::<gtk4::Expression>);

        let turbo_sw = gtk4::Switch::new();
        turbo_sw.set_active(true);
        turbo_sw.set_valign(gtk4::Align::Center);
        let turbo_lbl = Label::new(Some("Turbo"));

        let save_btn = Button::with_label("Save Profile");
        save_btn.add_css_class("suggested-action");

        let cfg_c   = self.config_manager.clone();
        let name_c  = name_entry.clone();
        let govs_c  = gov_names.clone();
        let gov_c   = gov_dd.clone();
        let turbo_c = turbo_sw.clone();
        save_btn.connect_clicked(move |btn| {
            let name = name_c.text().to_string().trim().to_string();
            if name.is_empty() { return; }
            let idx = gov_c.selected() as usize;
            let governor = govs_c.get(idx).cloned().unwrap_or_else(|| "powersave".into());
            let turbo_mode = if turbo_c.is_active() {
                crate::backend::profile::TurboMode::Always
            } else {
                crate::backend::profile::TurboMode::Never
            };
            let profile = Profile {
                name: name.clone(),
                description: format!("Custom: {} governor", governor),
                governor,
                turbo: turbo_mode,
                min_freq_mhz: None,
                max_freq_mhz: None,
                epp: None,
                epb: None,
            };
            let mut cfg = cfg_c.lock().unwrap();
            cfg.get_config_mut().custom_profiles.retain(|p| p.name != name);
            cfg.get_config_mut().custom_profiles.push(profile);
            if cfg.save().is_ok() {
                btn.set_label("✓ Saved — restart to apply");
            } else {
                btn.set_label("✗ Save failed");
            }
            let bc = btn.clone();
            glib::timeout_add_seconds_local(3, move || { bc.set_label("Save Profile"); glib::ControlFlow::Break });
        });

        form.append(&name_entry);
        form.append(&gov_dd);
        form.append(&turbo_lbl);
        form.append(&turbo_sw);
        form.append(&save_btn);
        vbox.append(&form);

        // List existing custom profiles
        let existing = self.config_manager.lock().unwrap()
            .get_config().custom_profiles.clone();
        for p in &existing {
            let row = Box::new(Orientation::Horizontal, 8);
            row.add_css_class("settings-row");
            let lbl = Label::new(Some(&format!("{} — {} governor", p.name, p.governor)));
            lbl.set_hexpand(true);
            let del_btn = Button::with_label("Remove");
            del_btn.add_css_class("destructive-action");
            let cfg_c2  = self.config_manager.clone();
            let pname_c = p.name.clone();
            del_btn.connect_clicked(move |btn| {
                let mut cfg = cfg_c2.lock().unwrap();
                cfg.get_config_mut().custom_profiles.retain(|p| p.name != pname_c);
                let _ = cfg.save();
                btn.set_label("✓ Removed — restart to apply");
            });
            row.append(&lbl);
            row.append(&del_btn);
            vbox.append(&row);
        }

        frame.set_child(Some(&vbox));
        frame
    }

    // ── System info ───────────────────────────────────────────────────────────────

    fn create_system_info(&self) -> Frame {
        let frame = Frame::new(Some("System Information"));
        frame.add_css_class("card");
        let vbox = Box::new(Orientation::Vertical, 6);
        vbox.set_margin_top(12); vbox.set_margin_bottom(12);
        vbox.set_margin_start(12); vbox.set_margin_end(12);

        let cpu = self.cpu_manager.lock().unwrap();
        let items = vec![
            ("CPU Model",  self.cpu_info.model.clone()),
            ("Vendor",     self.cpu_info.vendor.clone()),
            ("Cores",      self.cpu_info.core_count.to_string()),
            ("Driver",     format!("{:?}", self.cpu_info.driver)),
            ("HW Min",     format!("{} MHz", self.cpu_info.min_freq)),
            ("HW Max",     format!("{} MHz", self.cpu_info.max_freq)),
            ("Governors",  cpu.get_available_governors(0).unwrap_or_default().join(", ")),
            ("Thermal Zones", {
                let tm = self.thermal_manager.lock().unwrap();
                tm.get_zone_count().to_string()
            }),
            ("RAPL Available", RaplTracker::is_available().to_string()),
        ];

        for (label, value) in items {
            let row = Box::new(Orientation::Horizontal, 12);
            let l = Label::new(Some(&label)); l.add_css_class("subtitle"); l.set_halign(gtk4::Align::Start); l.set_hexpand(true);
            let v = Label::new(Some(&value)); v.set_halign(gtk4::Align::End);
            row.append(&l); row.append(&v);
            vbox.append(&row);
        }

        frame.set_child(Some(&vbox));
        frame
    }

    // ── Update loop ───────────────────────────────────────────────────────────────

    fn setup_updates(&self) {
        let freq_label    = self.freq_label.clone();
        let usage_label   = self.usage_label.clone();
        let temp_label    = self.temp_label.clone();
        let governor_label = self.governor_label.clone();
        let turbo_label   = self.turbo_label.clone();
        let power_label   = self.power_label.clone();
        let battery_label = self.battery_label.clone();
        let fan_label     = self.fan_label.clone();
        let cpu_manager   = self.cpu_manager.clone();
        let thermal_manager = self.thermal_manager.clone();
        let profile_manager = self.profile_manager.clone();
        let config_manager  = self.config_manager.clone();
        let cpu_usage_history = self.cpu_usage_history.clone();
        let cpu_usage_area    = self.cpu_usage_area.clone();
        let cpu_usage_tracker = self.cpu_usage_tracker.clone();
        let per_core_tracker  = self.per_core_tracker.clone();
        let rapl_tracker      = self.rapl_tracker.clone();
        let active_profile_label = self.active_profile_label.clone();
        let last_ac_status    = self.last_ac_status.clone();

        // Clone self fields needed for per-core rebuild
        let per_core_box  = self.per_core_box.clone();
        let cpu_manager2  = self.cpu_manager.clone();
        let thermal_manager2 = self.thermal_manager.clone();
        let per_core_tracker2 = self.per_core_tracker.clone();
        let cpu_info_core_count = self.cpu_info.core_count;

        glib::timeout_add_seconds_local(1, move || {
            let cpu_mgr = cpu_manager.lock().unwrap();

            // Average frequency
            if let Ok(freqs) = cpu_mgr.get_all_frequencies() {
                if !freqs.is_empty() {
                    let avg = freqs.iter().sum::<u32>() / freqs.len() as u32;
                    freq_label.set_text(&format!("{} MHz", avg));
                }
            }

            // Overall CPU usage
            let usage = cpu_usage_tracker.lock().unwrap().get_usage();
            usage_label.set_text(&format!("{:.0} %", usage));
            {
                let mut h = cpu_usage_history.lock().unwrap();
                h.remove(0); h.push(usage);
            }
            cpu_usage_area.queue_draw();

            // Temperature
            if let Ok(temp) = thermal_manager.lock().unwrap().get_cpu_temperature() {
                let css = if temp < 60.0 { "temp-normal" }
                    else if temp < 75.0 { "temp-warm" }
                    else if temp < 85.0 { "temp-hot" }
                    else { "temp-critical" };
                temp_label.set_text(&format!("{:.1}°C", temp));
                for c in &["temp-normal","temp-warm","temp-hot","temp-critical"] { temp_label.remove_css_class(c); }
                temp_label.add_css_class(css);

                // Critical temp notification
                let cfg = config_manager.lock().unwrap().get_config().clone();
                if cfg.general.critical_temp_notify && temp >= cfg.thermal.max_temp_celsius {
                    let _ = notify_rust::Notification::new()
                        .summary("CPU Temperature Critical!")
                        .body(&format!("CPU is at {:.1}°C — consider switching to Power Saver profile.", temp))
                        .icon("dialog-warning")
                        .timeout(notify_rust::Timeout::Milliseconds(5000))
                        .show();
                }
            }

            // Governor + Turbo
            if let Ok(gov) = cpu_mgr.get_governor(0) { governor_label.set_text(&gov); }
            if let Ok(turbo) = cpu_mgr.is_turbo_enabled() {
                turbo_label.set_text(if turbo { "Enabled" } else { "Disabled" });
                turbo_label.remove_css_class("status-ok"); turbo_label.remove_css_class("status-warning");
                turbo_label.add_css_class(if turbo { "status-ok" } else { "status-warning" });
            }

            // RAPL power
            if let Some(watts) = rapl_tracker.lock().unwrap().get_power_w() {
                power_label.set_text(&format!("{:.1} W", watts));
            } else if !RaplTracker::is_available() {
                power_label.set_text("N/A");
            }

            // Battery
            let bat = PowerSupplyReader::read();
            if bat.present {
                let charge_str = if let Some(w) = bat.power_now_w {
                    format!("{:.0}% · {} · {:.1}W", bat.charge_percent, bat.status, w)
                } else {
                    format!("{:.0}% · {}", bat.charge_percent, bat.status)
                };
                battery_label.set_text(&charge_str);
            } else {
                battery_label.set_text(if bat.on_ac { "AC Power" } else { "No Battery" });
            }

            // Fan
            if let Some(rpm) = HwmonReader::get_fan_rpm() {
                fan_label.set_text(&format!("{} RPM", rpm));
            } else {
                fan_label.set_text("N/A");
            }

            // AC/Battery auto-switch
            {
                let cfg = config_manager.lock().unwrap().get_config().clone();
                if cfg.auto_tune.enabled {
                    let on_ac = PowerSupplyReader::read().on_ac;
                    let mut last = last_ac_status.lock().unwrap();
                    if *last != Some(on_ac) {
                        *last = Some(on_ac);
                        let profile_name = if on_ac { &cfg.auto_tune.ac_profile } else { &cfg.auto_tune.battery_profile };
                        let pm = profile_manager.lock().unwrap();
                        if let Some(profile) = pm.get_profiles().iter().find(|p| p.name.to_lowercase() == profile_name.to_lowercase()).cloned() {
                            drop(pm);
                            match profile.apply(&cpu_mgr) {
                                Ok(_) => {
                                    active_profile_label.set_text(&profile.name);
                                    log::info!("Auto-switched to {} profile (AC: {})", profile.name, on_ac);
                                }
                                Err(e) => log::warn!("Auto-switch failed: {}", e),
                            }
                        }
                    }
                }
            }

            drop(cpu_mgr);

            // Per-core panel rebuild
            {
                let usages = per_core_tracker2.lock().unwrap().get_usage();
                let cpu2 = cpu_manager2.lock().unwrap();

                while let Some(child) = per_core_box.first_child() {
                    per_core_box.remove(&child);
                }
                let core_temps = HwmonReader::get_per_core_temps();
                let grid = Grid::new();
                grid.set_row_spacing(4); grid.set_column_spacing(8);

                if let Ok(statuses) = cpu2.get_all_core_status() {
                    for (i, status) in statuses.iter().enumerate() {
                        let is_p = cpu2.is_p_core(status.core_id);
                        let row_box = Box::new(Orientation::Horizontal, 8);
                        row_box.add_css_class("freq-display");
                        row_box.set_hexpand(true);

                        let core_lbl = Label::new(Some(&format!("Core {:2} [{}]", status.core_id, if is_p { "P" } else { "E" })));
                        core_lbl.set_width_chars(14);

                        let freq_lbl = Label::new(Some(&format!("{:4} MHz", status.current_freq)));
                        freq_lbl.add_css_class("value");
                        freq_lbl.set_width_chars(9);

                        let gov_lbl = Label::new(Some(&format!("[{}]", status.governor)));
                        gov_lbl.add_css_class("subtitle");
                        gov_lbl.set_width_chars(13);

                        let usage_pct = usages.get(i).copied().unwrap_or(0.0);
                        let bar = LevelBar::new();
                        bar.set_min_value(0.0); bar.set_max_value(100.0);
                        bar.set_value(usage_pct as f64);
                        bar.set_hexpand(true);
                        bar.set_valign(gtk4::Align::Center);

                        let pct_lbl = Label::new(Some(&format!("{:3.0}%", usage_pct)));
                        pct_lbl.set_width_chars(5);

                        row_box.append(&core_lbl);
                        row_box.append(&freq_lbl);
                        row_box.append(&gov_lbl);
                        row_box.append(&bar);
                        row_box.append(&pct_lbl);

                        // Per-core temp
                        if let Some(&(_, temp)) = core_temps.iter().find(|(id, _)| *id == status.core_id) {
                            let t = Label::new(Some(&format!("{:.0}°C", temp)));
                            t.add_css_class("subtitle"); t.set_width_chars(6);
                            row_box.append(&t);
                        }

                        // Online toggle
                        let online_sw = gtk4::Switch::new();
                        online_sw.set_active(status.online);
                        online_sw.set_sensitive(status.core_id != 0);
                        online_sw.set_valign(gtk4::Align::Center);
                        let cpu_cc = cpu_manager2.clone();
                        let cid = status.core_id;
                        online_sw.connect_state_set(move |_, state| {
                            let _ = cpu_cc.lock().unwrap().set_core_online(cid, state);
                            glib::Propagation::Proceed
                        });
                        row_box.append(&online_sw);

                        let col = (i % 2) as i32;
                        let row = (i / 2) as i32;
                        grid.attach(&row_box, col, row, 1, 1);
                    }
                }
                let _ = cpu_info_core_count; // suppress warning
                per_core_box.append(&grid);
            }

            glib::ControlFlow::Continue
        });
    }

    pub fn present(&self) { self.window.present(); }
    pub fn window_handle(&self) -> gtk4::ApplicationWindow { self.window.clone() }
}

/// Creates a macOS-style circular traffic-light dot drawn with Cairo.
/// Using DrawingArea instead of Button because GTK4 Button enforces internal
/// padding and minimum dimensions that prevent true circles via CSS alone.
fn traffic_dot(r: f64, g: f64, b: f64) -> gtk4::DrawingArea {
    use std::cell::Cell;
    use std::rc::Rc;

    let area = gtk4::DrawingArea::new();
    area.set_size_request(13, 13);
    area.set_content_width(13);
    area.set_content_height(13);
    area.set_valign(gtk4::Align::Center);
    area.set_cursor_from_name(Some("pointer"));

    let hovered = Rc::new(Cell::new(false));

    let h_draw = hovered.clone();
    area.set_draw_func(move |_a, cr, w, h| {
        let cx = w as f64 / 2.0;
        let cy = h as f64 / 2.0;
        let radius = (cx.min(cy) - 0.5).max(0.0);
        let alpha = if h_draw.get() { 0.65 } else { 1.0 };
        cr.set_source_rgba(r, g, b, alpha);
        let _ = cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    });

    let motion = gtk4::EventControllerMotion::new();
    let a1 = area.downgrade();
    let h1 = hovered.clone();
    motion.connect_enter(move |_, _, _| {
        h1.set(true);
        if let Some(a) = a1.upgrade() { a.queue_draw(); }
    });
    let a2 = area.downgrade();
    let h2 = hovered.clone();
    motion.connect_leave(move |_| {
        h2.set(false);
        if let Some(a) = a2.upgrade() { a.queue_draw(); }
    });
    area.add_controller(motion);

    area
}
