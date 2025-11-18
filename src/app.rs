use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box, Button, HeaderBar, Label, Orientation, Switch, ComboBoxText, Grid, ScrolledWindow, Frame};
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
        };

        app_window.setup_ui();
        app_window
    }

    fn setup_ui(&self) {
        // Create header bar
        let header = HeaderBar::new();
        header.set_show_title_buttons(true);
        
        let title_box = Box::new(Orientation::Vertical, 4);
        let title = Label::new(Some("CPU Power Manager"));
        title.add_css_class("title");
        let subtitle = Label::new(Some("Advanced CPU Control"));
        subtitle.add_css_class("subtitle");
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
                        // Reset label after 2 seconds
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

        // ============ NEW: Add Maximum Frequency Button ============
        let max_freq_box = Box::new(Orientation::Horizontal, 8);
        max_freq_box.set_halign(gtk4::Align::Center);
        max_freq_box.set_margin_top(12);

        let max_freq_button = Button::with_label("🚀 Maximum Frequency (All Cores)");
        max_freq_button.add_css_class("suggested-action");
        max_freq_button.set_tooltip_text(Some("Automatically detect and set to your CPU's maximum hardware frequency"));
        
        let cpu_manager_clone = self.cpu_manager.clone();
        max_freq_button.connect_clicked(move |btn| {
            let cpu_manager = cpu_manager_clone.lock().unwrap();
            
            // DYNAMICALLY READ hardware maximum frequency from CPU
            match cpu_manager.get_hardware_max_freq(0) {
                Ok(max_freq) => {
                    log::info!("Detected hardware maximum frequency: {} MHz (reading from /sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq)", max_freq);
                    log::info!("Setting all {} cores to maximum frequency: {} MHz", cpu_manager.core_count(), max_freq);
                    
                    // Get hardware minimum
                    let hw_min = match cpu_manager.get_hardware_min_freq(0) {
                        Ok(min) => {
                            log::debug!("Hardware minimum frequency: {} MHz", min);
                            min
                        },
                        Err(e) => {
                            log::error!("Failed to get hardware min freq: {}", e);
                            btn.set_label("✗ Error reading min freq");
                            let btn_clone = btn.clone();
                            glib::timeout_add_seconds_local(3, move || {
                                btn_clone.set_label("🚀 Maximum Frequency (All Cores)");
                                glib::ControlFlow::Break
                            });
                            return;
                        }
                    };

                    // Reset all limits to full hardware range (removes any caps)
                    log::info!("Resetting frequency range to hardware limits: {}-{} MHz", hw_min, max_freq);
                    for core in 0..cpu_manager.core_count() {
                        if let Err(e) = cpu_manager.set_scaling_min_freq(core, hw_min) {
                            log::warn!("Failed to reset min freq for core {}: {}", core, e);
                        }
                        if let Err(e) = cpu_manager.set_scaling_max_freq(core, max_freq) {
                            log::warn!("Failed to set max freq for core {}: {}", core, e);
                        }
                    }

                    // Set performance governor for best speed
                    if let Err(e) = cpu_manager.set_governor_all("performance") {
                        log::warn!("Failed to set performance governor: {}", e);
                    }

                    // Enable turbo boost
                    if let Err(e) = cpu_manager.set_turbo(true) {
                        log::warn!("Failed to enable turbo: {}", e);
                    }

                    btn.set_label(&format!("✓ Set to {} MHz", max_freq));
                    log::info!("Successfully configured all cores for maximum frequency: {} MHz", max_freq);
                    
                    // Reset button label after 3 seconds
                    let btn_clone = btn.clone();
                    glib::timeout_add_seconds_local(3, move || {
                        btn_clone.set_label("🚀 Maximum Frequency (All Cores)");
                        glib::ControlFlow::Break
                    });
                }
                Err(e) => {
                    log::error!("Failed to read hardware max frequency from CPU: {}", e);
                    btn.set_label("✗ Cannot read CPU max freq");
                    
                    let btn_clone = btn.clone();
                    glib::timeout_add_seconds_local(3, move || {
                        btn_clone.set_label("🚀 Maximum Frequency (All Cores)");
                        glib::ControlFlow::Break
                    });
                }
            }
        });

        max_freq_box.append(&max_freq_button);
        section.append(&max_freq_box);
        // ============ END NEW BUTTON ============

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

        // Governor selector
        let gov_label = Label::new(Some("Governor:"));
        gov_label.set_halign(gtk4::Align::End);
        grid.attach(&gov_label, 0, 0, 1, 1);

        let governor_combo = ComboBoxText::new();
        if let Ok(governors) = cpu_manager.get_available_governors(0) {
            for gov in governors {
                governor_combo.append_text(&gov);
            }
            if let Ok(current) = cpu_manager.get_governor(0) {
                governor_combo.set_active_id(Some(&current));
            }
        }

        let cpu_mgr_clone = self.cpu_manager.clone();
        governor_combo.connect_changed(move |combo| {
            if let Some(governor) = combo.active_text() {
                let cpu_manager = cpu_mgr_clone.lock().unwrap();
                if let Err(e) = cpu_manager.set_governor_all(&governor) {
                    log::error!("Failed to set governor: {}", e);
                }
            }
        });
        grid.attach(&governor_combo, 1, 0, 1, 1);

        // Turbo boost toggle
        let turbo_label = Label::new(Some("Turbo Boost:"));
        turbo_label.set_halign(gtk4::Align::End);
        grid.attach(&turbo_label, 0, 1, 1, 1);

        let turbo_switch = Switch::new();
        if let Ok(enabled) = cpu_manager.is_turbo_enabled() {
            turbo_switch.set_active(enabled);
        }

        let cpu_mgr_clone = self.cpu_manager.clone();
        turbo_switch.connect_state_set(move |_, state| {
            let cpu_manager = cpu_mgr_clone.lock().unwrap();
            if let Err(e) = cpu_manager.set_turbo(state) {
                log::error!("Failed to set turbo: {}", e);
            }
            glib::Propagation::Proceed
        });
        grid.attach(&turbo_switch, 1, 1, 1, 1);

        // Info label
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
        
        // Available Governors
        if let Ok(governors) = cpu_manager.get_available_governors(0) {
            let gov_label = Label::new(Some(&format!("Available Governors: {}", governors.join(", "))));
            status_box.append(&gov_label);
        }

        status_box
    }

    fn setup_updates(&self) {
        // Setup periodic UI updates every second
        let freq_label = self.freq_label.clone();
        let temp_label = self.temp_label.clone();
        let governor_label = self.governor_label.clone();
        let turbo_label = self.turbo_label.clone();
        let cpu_manager = self.cpu_manager.clone();
        let thermal_manager = self.thermal_manager.clone();
        let per_core_box = self.per_core_box.clone();
        let cpu_mgr_clone = self.cpu_manager.clone();

        glib::timeout_add_seconds_local(1, move || {
            // Update frequency
            let cpu_mgr = cpu_manager.lock().unwrap();
            if let Ok(freqs) = cpu_mgr.get_all_frequencies() {
                let avg_freq = freqs.iter().sum::<u32>() / freqs.len() as u32;
                freq_label.set_text(&format!("{} MHz", avg_freq));
            }

            // Update temperature
            let thermal_mgr = thermal_manager.lock().unwrap();
            if let Ok(temp) = thermal_mgr.get_cpu_temperature() {
                temp_label.set_text(&format!("{:.1}°C", temp));
                
                // Update CSS class based on temperature
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

            // Update governor
            if let Ok(gov) = cpu_mgr.get_governor(0) {
                governor_label.set_text(&gov);
            }

            // Update turbo
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

        // Update per-core display every 2 seconds (heavier operation)
        let cpu_mgr_clone2 = cpu_mgr_clone.clone();
        glib::timeout_add_seconds_local(2, move || {
            let cpu_mgr = cpu_mgr_clone2.lock().unwrap();
            
            // Clear existing
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
    }

    pub fn present(&self) {
        self.window.present();
    }
}
