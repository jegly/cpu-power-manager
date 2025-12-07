use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box, Button, HeaderBar, Label, Orientation, Switch, DropDown, StringList, Grid, ScrolledWindow, Frame};
use crate::backend::CpuManager;
use crate::backend::thermal::ThermalManager;
use crate::backend::profile::ProfileManager;
use crate::config::ConfigManager;
use std::sync::{Arc, Mutex};

pub struct AppWindow {
    window: ApplicationWindow,
    cpu_manager: Arc<Mutex<CpuManager>>,
    thermal_manager: Arc<Mutex<ThermalManager>>,
    profile_manager: Arc<Mutex<ProfileManager>>,
    config_manager: Arc<Mutex<ConfigManager>>,
    // UI elements we need to update
    freq_label: Label,
    temp_label: Label,
    governor_label: Label,
    turbo_label: Label,
    per_core_box: Box,
    cpu_usage_area: gtk4::DrawingArea,
    cpu_usage_history: Arc<Mutex<Vec<f32>>>,
}

impl AppWindow {
    pub fn new(app: &Application) -> Self {
        let cpu_manager = Arc::new(Mutex::new(
            CpuManager::new().expect("Failed to initialize CPU manager")
        ));
        let thermal_manager = Arc::new(Mutex::new(
            ThermalManager::new().expect("Failed to initialize thermal manager")
        ));
        let profile_manager = Arc::new(Mutex::new(ProfileManager::new()));
        let config_manager = Arc::new(Mutex::new(
            ConfigManager::new().expect("Failed to initialize config manager")
        ));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("CPU Power Manager")
            .default_width(1000)
            .default_height(700)
            .build();

        // Create labels that we'll update
        let freq_label = Label::new(Some("-- MHz"));
        let temp_label = Label::new(Some("--°C"));
        let governor_label = Label::new(Some("--"));
        let turbo_label = Label::new(Some("--"));
        let per_core_box = Box::new(Orientation::Vertical, 4);
        
        // CPU usage graph
        let cpu_usage_area = gtk4::DrawingArea::new();
        cpu_usage_area.set_content_width(600);
        cpu_usage_area.set_content_height(200);
        let cpu_usage_history = Arc::new(Mutex::new(vec![0.0; 60])); // 60 seconds of history

        let app_window = Self {
            window,
            cpu_manager,
            thermal_manager,
            profile_manager,
            config_manager,
            freq_label,
            temp_label,
            governor_label,
            turbo_label,
            per_core_box,
            cpu_usage_area,
            cpu_usage_history,
        };

        app_window.setup_ui();
        app_window
    }

    fn setup_ui(&self) {
        // Create header bar
        let header = HeaderBar::new();
        // Disable default minimize/maximize/close buttons
        header.set_show_title_buttons(false);
        
        // Add macOS-style traffic lights to the left
        let traffic_box = Box::new(Orientation::Horizontal, 8);
        traffic_box.set_margin_start(12);
        
        // Close button (red)
        let close_btn = Button::new();
        close_btn.set_size_request(12, 12);
        close_btn.add_css_class("traffic-btn");
        close_btn.add_css_class("traffic-close");
        let window_clone = self.window.clone();
        close_btn.connect_clicked(move |_| {
            window_clone.close();
        });
        
        // Minimize button (yellow)
        let min_btn = Button::new();
        min_btn.set_size_request(12, 12);
        min_btn.add_css_class("traffic-btn");
        min_btn.add_css_class("traffic-minimize");
        let window_clone = self.window.clone();
        min_btn.connect_clicked(move |_| {
            window_clone.minimize();
        });
        
        // Maximize button (green)
        let max_btn = Button::new();
        max_btn.set_size_request(12, 12);
        max_btn.add_css_class("traffic-btn");
        max_btn.add_css_class("traffic-maximize");
        let window_clone = self.window.clone();
        max_btn.connect_clicked(move |_| {
            if window_clone.is_maximized() {
                window_clone.unmaximize();
            } else {
                window_clone.maximize();
            }
        });
        
        traffic_box.append(&close_btn);
        traffic_box.append(&min_btn);
        traffic_box.append(&max_btn);
        
        header.pack_start(&traffic_box);
        
        // Title in center
        let title_box = Box::new(Orientation::Vertical, 0);
        let title = Label::new(Some("CPU Power Manager"));
        title.add_css_class("header-title");
        let subtitle = Label::new(Some("Advanced CPU Control"));
        subtitle.add_css_class("header-subtitle");
        title_box.append(&title);
        title_box.append(&subtitle);
        header.set_title_widget(Some(&title_box));

        self.window.set_titlebar(Some(&header));

        // Create scrolled window for main content
        let scrolled = ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hexpand(true);

        // Main container
        let main_box = Box::new(Orientation::Vertical, 12);
        main_box.set_margin_top(12);
        main_box.set_margin_bottom(12);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);

        // Dashboard section
        let dashboard = self.create_dashboard();
        main_box.append(&dashboard);

        // CPU Usage Graph section
        let cpu_graph = self.create_cpu_usage_graph();
        main_box.append(&cpu_graph);

        // Quick Profile buttons
        let profiles_box = self.create_profile_buttons();
        main_box.append(&profiles_box);

        // Advanced Controls section
        let controls = self.create_advanced_controls();
        main_box.append(&controls);

        // Per-Core Status section
        let per_core_section = self.create_per_core_section();
        main_box.append(&per_core_section);

        // Status section
        let status = self.create_status_section();
        main_box.append(&status);

        scrolled.set_child(Some(&main_box));
        self.window.set_child(Some(&scrolled));

        // Setup periodic updates
        self.setup_updates();
    }

    fn create_dashboard(&self) -> Box {
        let dashboard = Box::new(Orientation::Horizontal, 12);
        dashboard.add_css_class("card");

        // CPU Info
        let cpu_box = Box::new(Orientation::Vertical, 8);
        let cpu_title = Label::new(Some("CPU Information"));
        cpu_title.add_css_class("title");
        cpu_box.append(&cpu_title);

        let cpu_manager = self.cpu_manager.lock().unwrap();
        if let Ok(info) = cpu_manager.get_cpu_info() {
            let model_label = Label::new(Some(&format!("Model: {}", info.model)));
            let cores_label = Label::new(Some(&format!("Cores: {}", info.core_count)));
            let driver_label = Label::new(Some(&format!("Driver: {:?}", info.driver)));
            let hw_range_label = Label::new(Some(&format!("HW Range: {} - {} MHz", info.min_freq, info.max_freq)));
            
            cpu_box.append(&model_label);
            cpu_box.append(&cores_label);
            cpu_box.append(&driver_label);
            cpu_box.append(&hw_range_label);
        }

        dashboard.append(&cpu_box);

        // Frequency Info
        let freq_box = Box::new(Orientation::Vertical, 8);
        let freq_title = Label::new(Some("Average Frequency"));
        freq_title.add_css_class("title");
        freq_box.append(&freq_title);
        
        self.freq_label.add_css_class("freq-value");
        freq_box.append(&self.freq_label);

        dashboard.append(&freq_box);

        // Temperature Info
        let temp_box = Box::new(Orientation::Vertical, 8);
        let temp_title = Label::new(Some("CPU Temperature"));
        temp_title.add_css_class("title");
        temp_box.append(&temp_title);
        
        temp_box.append(&self.temp_label);

        dashboard.append(&temp_box);

        // Governor Info
        let gov_box = Box::new(Orientation::Vertical, 8);
        let gov_title = Label::new(Some("Current Governor"));
        gov_title.add_css_class("title");
        gov_box.append(&gov_title);
        
        gov_box.append(&self.governor_label);

        dashboard.append(&gov_box);

        // Turbo Info
        let turbo_box = Box::new(Orientation::Vertical, 8);
        let turbo_title = Label::new(Some("Turbo Boost"));
        turbo_title.add_css_class("title");
        turbo_box.append(&turbo_title);
        
        turbo_box.append(&self.turbo_label);

        dashboard.append(&turbo_box);

        dashboard
    }

    fn create_cpu_usage_graph(&self) -> Frame {
        let frame = Frame::new(Some("CPU Usage History"));
        frame.add_css_class("card");

        let graph_box = Box::new(Orientation::Vertical, 8);
        graph_box.set_margin_top(12);
        graph_box.set_margin_bottom(12);
        graph_box.set_margin_start(12);
        graph_box.set_margin_end(12);

        // Setup drawing function
        let history_clone = self.cpu_usage_history.clone();
        self.cpu_usage_area.set_draw_func(move |_area, cr, width, height| {
            let history = history_clone.lock().unwrap();
            
            // Background
            cr.set_source_rgb(0.08, 0.08, 0.08);
            let _ = cr.paint();
            
            // Grid lines
            cr.set_source_rgba(0.2, 0.2, 0.2, 0.5);
            cr.set_line_width(1.0);
            for i in 0..5 {
                let y = (i as f64 / 4.0) * height as f64;
                let _ = cr.move_to(0.0, y);
                let _ = cr.line_to(width as f64, y);
                let _ = cr.stroke();
            }
            
            // Draw usage graph
            if history.len() > 1 {
                let point_spacing = width as f64 / (history.len() - 1) as f64;
                
                // Create gradient fill
                cr.set_source_rgba(0.23, 0.51, 0.96, 0.3);
                let _ = cr.move_to(0.0, height as f64);
                
                for (i, &usage) in history.iter().enumerate() {
                    let x = i as f64 * point_spacing;
                    let y = height as f64 - (usage as f64 / 100.0 * height as f64);
                    let _ = cr.line_to(x, y);
                }
                
                let _ = cr.line_to(width as f64, height as f64);
                let _ = cr.close_path();
                let _ = cr.fill();
                
                // Draw line
                cr.set_source_rgb(0.23, 0.51, 0.96);
                cr.set_line_width(2.5);
                let _ = cr.move_to(0.0, height as f64 - (history[0] as f64 / 100.0 * height as f64));
                
                for (i, &usage) in history.iter().enumerate() {
                    let x = i as f64 * point_spacing;
                    let y = height as f64 - (usage as f64 / 100.0 * height as f64);
                    let _ = cr.line_to(x, y);
                }
                let _ = cr.stroke();
            }
        });

        graph_box.append(&self.cpu_usage_area);
        
        // Add labels
        let info_box = Box::new(Orientation::Horizontal, 12);
        info_box.set_halign(gtk4::Align::Center);
        
        let label_100 = Label::new(Some("100%"));
        label_100.add_css_class("subtitle");
        let label_0 = Label::new(Some("0%"));
        label_0.add_css_class("subtitle");
        let label_time = Label::new(Some("← 60s history"));
        label_time.add_css_class("subtitle");
        
        info_box.append(&label_100);
        info_box.append(&label_time);
        info_box.append(&label_0);
        
        graph_box.append(&info_box);
        frame.set_child(Some(&graph_box));
        frame
    }

    fn create_profile_buttons(&self) -> Box {
        let section = Box::new(Orientation::Vertical, 8);
        
        let title = Label::new(Some("Quick Profiles"));
        title.add_css_class("title");
        section.append(&title);

        let profiles_box = Box::new(Orientation::Horizontal, 8);
        profiles_box.set_halign(gtk4::Align::Center);

        let profile_manager = self.profile_manager.lock().unwrap();
        for profile in profile_manager.get_profiles() {
            let button = Button::with_label(&profile.name);
            button.set_tooltip_text(Some(&profile.description));
            
            let cpu_manager = self.cpu_manager.clone();
            let profile_clone = profile.clone();
            button.connect_clicked(move |btn| {
                let cpu_manager = cpu_manager.lock().unwrap();
                match profile_clone.apply(&cpu_manager) {
                    Ok(_) => {
                        btn.set_label(&format!("✓ {}", profile_clone.name));
                        let btn_clone = btn.clone();
                        let name = profile_clone.name.clone();
                        glib::timeout_add_seconds_local(2, move || {
                            btn_clone.set_label(&name);
                            glib::ControlFlow::Break
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to apply profile: {}", e);
                        btn.set_label(&format!("✗ {}", profile_clone.name));
                        let btn_clone = btn.clone();
                        let name = profile_clone.name.clone();
                        glib::timeout_add_seconds_local(2, move || {
                            btn_clone.set_label(&name);
                            glib::ControlFlow::Break
                        });
                    }
                }
            });

            profiles_box.append(&button);
        }

        section.append(&profiles_box);

        let max_freq_box = Box::new(Orientation::Horizontal, 8);
        max_freq_box.set_halign(gtk4::Align::Center);
        max_freq_box.set_margin_top(12);

        let max_freq_button = Button::with_label("Maximum Frequency (All Cores)");
        max_freq_button.add_css_class("suggested-action");
        max_freq_button.set_tooltip_text(Some("Automatically detect and set to your CPU's maximum hardware frequency"));
        
        let cpu_manager_clone = self.cpu_manager.clone();
        max_freq_button.connect_clicked(move |btn| {
            let cpu_manager = cpu_manager_clone.lock().unwrap();
            
            match cpu_manager.get_hardware_max_freq(0) {
                Ok(max_freq) => {
                    log::info!("Detected hardware maximum frequency: {} MHz", max_freq);
                    
                    let hw_min = match cpu_manager.get_hardware_min_freq(0) {
                        Ok(min) => min,
                        Err(e) => {
                            log::error!("Failed to get hardware min freq: {}", e);
                            btn.set_label("✗ Error reading min freq");
                            let btn_clone = btn.clone();
                            glib::timeout_add_seconds_local(3, move || {
                                btn_clone.set_label("Maximum Frequency (All Cores)");
                                glib::ControlFlow::Break
                            });
                            return;
                        }
                    };

                    for core in 0..cpu_manager.core_count() {
                        let _ = cpu_manager.set_scaling_min_freq(core, hw_min);
                        let _ = cpu_manager.set_scaling_max_freq(core, max_freq);
                    }

                    let _ = cpu_manager.set_governor_all("performance");
                    let _ = cpu_manager.set_turbo(true);

                    btn.set_label(&format!("✓ Set to {} MHz", max_freq));
                    
                    let btn_clone = btn.clone();
                    glib::timeout_add_seconds_local(3, move || {
                        btn_clone.set_label("Maximum Frequency (All Cores)");
                        glib::ControlFlow::Break
                    });
                }
                Err(e) => {
                    log::error!("Failed to read hardware max frequency: {}", e);
                    btn.set_label("✗ Cannot read CPU max freq");
                    
                    let btn_clone = btn.clone();
                    glib::timeout_add_seconds_local(3, move || {
                        btn_clone.set_label("Maximum Frequency (All Cores)");
                        glib::ControlFlow::Break
                    });
                }
            }
        });

        max_freq_box.append(&max_freq_button);
        section.append(&max_freq_box);

        section
    }

    fn create_advanced_controls(&self) -> Frame {
        let frame = Frame::new(Some("Advanced Controls"));
        frame.add_css_class("card");

        let grid = Grid::new();
        grid.set_row_spacing(12);
        grid.set_column_spacing(12);
        grid.set_margin_top(12);
        grid.set_margin_bottom(12);
        grid.set_margin_start(12);
        grid.set_margin_end(12);

        let cpu_manager = self.cpu_manager.lock().unwrap();

        let gov_label = Label::new(Some("Governor:"));
        gov_label.set_halign(gtk4::Align::End);
        grid.attach(&gov_label, 0, 0, 1, 1);

        let governor_combo = DropDown::new(None::<StringList>, None::<gtk4::Expression>);
        if let Ok(governors) = cpu_manager.get_available_governors(0) {
            let string_list = StringList::new(&governors.iter().map(|s| s.as_str()).collect::<Vec<_>>());
            governor_combo.set_model(Some(&string_list));
            
            if let Ok(current) = cpu_manager.get_governor(0) {
                if let Some(pos) = governors.iter().position(|g| g == &current) {
                    governor_combo.set_selected(pos as u32);
                }
            }
        }

        let cpu_mgr_clone = self.cpu_manager.clone();
        let governors_clone = cpu_manager.get_available_governors(0).unwrap_or_default();
        governor_combo.connect_selected_notify(move |combo| {
            let selected = combo.selected() as usize;
            if selected < governors_clone.len() {
                let governor = &governors_clone[selected];
                let cpu_manager = cpu_mgr_clone.lock().unwrap();
                if let Err(e) = cpu_manager.set_governor_all(governor) {
                    log::error!("Failed to set governor: {}", e);
                }
            }
        });
        grid.attach(&governor_combo, 1, 0, 1, 1);

        let turbo_label = Label::new(Some("Turbo Boost:"));
        turbo_label.set_halign(gtk4::Align::End);
        grid.attach(&turbo_label, 0, 1, 1, 1);

        let turbo_box = Box::new(Orientation::Horizontal, 8);
        let turbo_switch = Switch::new();
        turbo_switch.add_css_class("turbo-switch");
        if let Ok(enabled) = cpu_manager.is_turbo_enabled() {
            turbo_switch.set_active(enabled);
        }

        let turbo_status_label = Label::new(Some(if turbo_switch.is_active() { "ON" } else { "OFF" }));
        turbo_status_label.add_css_class("turbo-status");
        if turbo_switch.is_active() {
            turbo_status_label.add_css_class("turbo-on");
        } else {
            turbo_status_label.add_css_class("turbo-off");
        }

        let cpu_mgr_clone = self.cpu_manager.clone();
        let status_label_clone = turbo_status_label.clone();
        turbo_switch.connect_state_set(move |_sw, state| {
            let cpu_manager = cpu_mgr_clone.lock().unwrap();
            if let Err(e) = cpu_manager.set_turbo(state) {
                log::error!("Failed to set turbo: {}", e);
            }
            status_label_clone.set_text(if state { "ON" } else { "OFF" });
            status_label_clone.remove_css_class("turbo-on");
            status_label_clone.remove_css_class("turbo-off");
            if state {
                status_label_clone.add_css_class("turbo-on");
            } else {
                status_label_clone.add_css_class("turbo-off");
            }
            glib::Propagation::Proceed
        });
        
        turbo_box.append(&turbo_switch);
        turbo_box.append(&turbo_status_label);
        grid.attach(&turbo_box, 1, 1, 1, 1);

        let info_label = Label::new(Some("Note: Changes require root privileges. Run with sudo or configure PolicyKit."));
        info_label.add_css_class("subtitle");
        info_label.set_wrap(true);
        info_label.set_max_width_chars(60);
        grid.attach(&info_label, 0, 2, 2, 1);

        frame.set_child(Some(&grid));
        frame
    }

    fn create_per_core_section(&self) -> Frame {
        let frame = Frame::new(Some("Per-Core Status"));
        frame.add_css_class("card");

        self.per_core_box.set_margin_top(12);
        self.per_core_box.set_margin_bottom(12);
        self.per_core_box.set_margin_start(12);
        self.per_core_box.set_margin_end(12);

        frame.set_child(Some(&self.per_core_box));
        frame
    }

    fn create_status_section(&self) -> Box {
        let status_box = Box::new(Orientation::Vertical, 8);
        status_box.add_css_class("card");

        let status_title = Label::new(Some("System Information"));
        status_title.add_css_class("title");
        status_box.append(&status_title);

        let cpu_manager = self.cpu_manager.lock().unwrap();
        
        if let Ok(governors) = cpu_manager.get_available_governors(0) {
            let gov_label = Label::new(Some(&format!("Available Governors: {}", governors.join(", "))));
            status_box.append(&gov_label);
        }

        status_box
    }

    fn setup_updates(&self) {
        let freq_label = self.freq_label.clone();
        let temp_label = self.temp_label.clone();
        let governor_label = self.governor_label.clone();
        let turbo_label = self.turbo_label.clone();
        let cpu_manager = self.cpu_manager.clone();
        let thermal_manager = self.thermal_manager.clone();
        let per_core_box = self.per_core_box.clone();
        let cpu_mgr_clone = self.cpu_manager.clone();

        glib::timeout_add_seconds_local(1, move || {
            let cpu_mgr = cpu_manager.lock().unwrap();
            if let Ok(freqs) = cpu_mgr.get_all_frequencies() {
                let avg_freq = freqs.iter().sum::<u32>() / freqs.len() as u32;
                freq_label.set_text(&format!("{} MHz", avg_freq));
            }

            let thermal_mgr = thermal_manager.lock().unwrap();
            if let Ok(temp) = thermal_mgr.get_cpu_temperature() {
                temp_label.set_text(&format!("{:.1}°C", temp));
                
                temp_label.remove_css_class("temp-normal");
                temp_label.remove_css_class("temp-warm");
                temp_label.remove_css_class("temp-hot");
                temp_label.remove_css_class("temp-critical");
                
                if temp < 60.0 {
                    temp_label.add_css_class("temp-normal");
                } else if temp < 75.0 {
                    temp_label.add_css_class("temp-warm");
                } else if temp < 85.0 {
                    temp_label.add_css_class("temp-hot");
                } else {
                    temp_label.add_css_class("temp-critical");
                }
            }

            if let Ok(gov) = cpu_mgr.get_governor(0) {
                governor_label.set_text(&gov);
            }

            if let Ok(turbo) = cpu_mgr.is_turbo_enabled() {
                turbo_label.set_text(if turbo { "Enabled" } else { "Disabled" });
                if turbo {
                    turbo_label.add_css_class("status-ok");
                    turbo_label.remove_css_class("status-warning");
                } else {
                    turbo_label.add_css_class("status-warning");
                    turbo_label.remove_css_class("status-ok");
                }
            }

            glib::ControlFlow::Continue
        });

        let cpu_mgr_clone2 = cpu_mgr_clone.clone();
        glib::timeout_add_seconds_local(2, move || {
            let cpu_mgr = cpu_mgr_clone2.lock().unwrap();
            
            while let Some(child) = per_core_box.first_child() {
                per_core_box.remove(&child);
            }

            if let Ok(statuses) = cpu_mgr.get_all_core_status() {
                for status in statuses {
                    let core_box = Box::new(Orientation::Horizontal, 8);
                    core_box.add_css_class("freq-display");

                    let core_label = Label::new(Some(&format!("Core {}: ", status.core_id)));
                    let freq_label = Label::new(Some(&format!("{} MHz", status.current_freq)));
                    freq_label.add_css_class("value");
                    let gov_label = Label::new(Some(&format!("({})", status.governor)));
                    gov_label.add_css_class("subtitle");

                    core_box.append(&core_label);
                    core_box.append(&freq_label);
                    core_box.append(&gov_label);

                    per_core_box.append(&core_box);
                }
            }

            glib::ControlFlow::Continue
        });

        let cpu_usage_history = self.cpu_usage_history.clone();
        let cpu_usage_area = self.cpu_usage_area.clone();
        let cpu_mgr_clone3 = self.cpu_manager.clone();
        
        glib::timeout_add_seconds_local(1, move || {
            let cpu_mgr = cpu_mgr_clone3.lock().unwrap();
            
            if let Ok(freqs) = cpu_mgr.get_all_frequencies() {
                if let Ok(info) = cpu_mgr.get_cpu_info() {
                    let avg_freq = freqs.iter().sum::<u32>() / freqs.len() as u32;
                    let usage_percent = ((avg_freq as f32 / info.max_freq as f32) * 100.0).min(100.0);
                    
                    let mut history = cpu_usage_history.lock().unwrap();
                    history.remove(0);
                    history.push(usage_percent);
                    
                    cpu_usage_area.queue_draw();
                }
            }
            
            glib::ControlFlow::Continue
        });
    }

    pub fn present(&self) {
        self.window.present();
    }
}
