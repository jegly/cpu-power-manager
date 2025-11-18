# CPU Power Manager in BETA !

> Advanced CPU frequency and power management tool for Linux with GTK4 interface and Dracula theme

![License](https://img.shields.io/badge/license-GPL--3.0-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux-lightgrey.svg)

## Features

### Core Functionality
- ✅ **CPU Frequency Control**: Set fixed frequencies or dynamic scaling
- ✅ **Governor Management**: Switch between performance, powersave, schedutil, ondemand, and conservative
- ✅ **Turbo Boost Control**: Enable/disable Intel Turbo Boost or AMD Turbo Core
- ✅ **Per-Core Control**: Individual frequency and governor settings for each core
- ✅ **Hardware Limits**: Respect and display CPU hardware frequency limits

### Advanced Features
- 🔥 **Intelligent Auto-Tuning**: Automatic power profile switching based on:
  - AC vs Battery power state
  - CPU load and temperature
  - Adaptive polling with battery discharge analysis
  
- 🌡️ **Thermal Management**:
  - Multi-zone temperature monitoring
  - Thermal throttling protection
  - Temperature-based frequency adjustment
  - Real-time temperature graphs
  
- 📊 **Real-Time Monitoring**:
  - Live frequency graphs for all cores
  - CPU usage visualization
  - Temperature monitoring
  - Power consumption tracking
  
- 🎯 **Profile System**:
  - Pre-configured profiles: Performance, Balanced, Power Saver, Silent
  - Custom profile creation
  - One-click profile switching
  - Profile scheduling support
  
- 🎨 **Beautiful Interface**:
  - Modern GTK4 interface
  - Dracula color theme
  - Responsive design
  - System tray integration

### Safety Features
- 🛡️ Automatic fallback to safe values on errors
- 🛡️ Hardware limit enforcement
- 🛡️ Thermal protection
- 🛡️ Configuration validation
- 🛡️ Backup/restore functionality

## Screenshots

![Dashboard](screenshots/dashboard.png)
![Profile Selection](screenshots/profiles.png)

## System Requirements

- Linux kernel 4.4 or newer with cpufreq support
- GTK4 4.10+
- libadwaita 1.5+
- PolicyKit (for privilege escalation)
- Intel or AMD CPU with frequency scaling support

### Supported Drivers
- `intel_pstate` (Intel processors)
- `amd_pstate` (AMD processors)
- `acpi-cpufreq` (fallback for older systems)

## Installation

### From .deb Package (Debian/Ubuntu)

```bash
# Download the latest release
wget https://github.com/globalcve/cpu-power-manager/releases/latest/download/cpu-power-manager_1.0.0_amd64.deb

# Install
sudo dpkg -i cpu-power-manager_1.0.0_amd64.deb
sudo apt-get install -f  # Install dependencies if needed
```

### From Source

#### Install Build Dependencies

**Debian/Ubuntu:**
```bash
sudo apt install build-essential cargo rustc libgtk-4-dev \
    libadwaita-1-dev libglib2.0-dev pkg-config policykit-1 
## then install the below ##
rustup 
after installing rustup set to # rustup install nightly
  # rustup override set nightly
```

**Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel glib2-devel \
    rust cargo pkgconfig polkit
```

**Arch Linux:**
```bash
sudo pacman -S base-devel rust gtk4 libadwaita pkgconf polkit
```

#### Build and Install

```bash
# Clone the repository
git clone https://github.com/yourusername/cpu-power-manager.git
cd cpu-power-manager

# Build release version
cargo build --release

# Install
sudo cp target/release/cpu-power-manager /usr/local/bin/
sudo cp assets/cpu-power-manager.desktop /usr/share/applications/
sudo cp assets/com.cpupowermanager.policy /usr/share/polkit-1/actions/
sudo cp assets/icon.svg /usr/share/icons/hicolor/scalable/apps/cpu-power-manager.svg

# Update icon cache
sudo gtk-update-icon-cache /usr/share/icons/hicolor/
```

### Building .deb Package

```bash
cargo install cargo-deb
cargo deb
# Package will be in target/debian/
```

## Usage

### Graphical Interface

Launch the application from your application menu or run:
```bash
cpu-power-manager
```

### Command Line

```bash
# Show current CPU status
cpu-power-manager status

# Set governor for all cores
cpu-power-manager set-governor performance

# Set frequency (in MHz)
cpu-power-manager set-frequency 3000

# Enable/disable turbo boost
cpu-power-manager set-turbo true

# Apply a profile
cpu-power-manager apply-profile balanced

# Start background service
cpu-power-manager service

# Show version
cpu-power-manager version
```

## Configuration

Configuration file location: `~/.config/cpu-power-manager/config.toml`

Example configuration:

```toml
[general]
auto_start = true
start_minimized = false
polling_interval_ms = 1000
temperature_unit = "celsius"
notification_level = "important"

[auto_tune]
enabled = true
ac_profile = "performance"
battery_profile = "balanced"
temp_threshold_high = 80
temp_threshold_low = 60
load_threshold_high = 70
load_threshold_low = 30

[thermal]
max_temp_celsius = 90
emergency_temp_celsius = 95
fan_control_enabled = false

[monitoring]
enable_graphs = true
graph_history_seconds = 300
show_per_core_stats = true

[logging]
log_level = "info"
log_to_file = true
log_path = "~/.local/share/cpu-power-manager/app.log"
max_log_size_mb = 10
```

## Profiles

### Built-in Profiles

#### Performance
- **Governor**: performance
- **Turbo**: Always enabled
- **Best for**: Gaming, video editing, compilation
- **Trade-off**: Highest power consumption and heat

#### Balanced
- **Governor**: schedutil
- **Turbo**: Auto (load-based)
- **Best for**: Daily use, general productivity
- **Trade-off**: Good balance of performance and efficiency

#### Power Saver
- **Governor**: powersave
- **Turbo**: Disabled
- **Best for**: Battery life, light tasks
- **Trade-off**: Reduced performance

#### Silent
- **Governor**: powersave
- **Turbo**: Disabled
- **Max Frequency**: Limited to 2000 MHz
- **Best for**: Quiet operation, presentations
- **Trade-off**: Significantly reduced performance

## Troubleshooting

### Application won't start
- Check that GTK4 and libadwaita are installed
- Run with `--debug` flag to see detailed logs
- Ensure your system supports cpufreq

### Can't change frequency/governor
- Make sure PolicyKit is installed and running
- Check that you're in the appropriate group (usually `wheel` or `sudo`)
- Verify cpufreq driver is loaded: `cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_driver`

### Turbo boost not available
- Check if your CPU supports turbo boost
- Verify in BIOS that turbo is enabled
- Some laptops disable turbo in battery mode via BIOS

### Temperature not showing
- Install `lm-sensors` package
- Run `sudo sensors-detect` to configure sensors
- Check `/sys/class/thermal/` for available thermal zones

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by [auto-cpufreq](https://github.com/AdnanHodzic/auto-cpufreq)
- Inspired by [Watt](https://github.com/NotAShelf/watt)
- Dracula Theme by [Dracula](https://draculatheme.com/)
- GTK4 and libadwaita by [GNOME](https://www.gnome.org/)

## Support

- **Issues**: [GitHub Issues](https://github.com/globalcve/cpu-power-manager/issues)
- **Discussions**: [GitHub Discussions](https://github.com/globalcve/cpu-power-manager/discussions)

## Roadmap

- [ ] Fan control integration
- [ ] GPU frequency management
- [ ] Profile import/export
- [ ] System tray notifications
- [ ] Wayland-native implementation
- [ ] Multi-language support
- [ ] Integration with power-profiles-daemon
- [ ] Machine learning-based auto-tuning

---

**Note**: This tool requires root privileges to change CPU frequencies. Please use with caution and understand the implications of manually controlling CPU frequencies.

### Build Requirements

- Rust **nightly** toolchain (edition 2024 support required)
  ```bash
  rustup install nightly
  rustup override set nightly



