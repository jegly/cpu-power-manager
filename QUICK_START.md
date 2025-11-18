# Quick Start Guide

## Installation

### Option 1: Install from Release `.deb` (Recommended for Debian/Ubuntu)
Download the latest `.deb` from [Releases](https://github.com/globalcve/cpu-power-manager/releases):
wget https://github.com/globalcve/cpu-power-manager/releases/download/1.0.0-1/cpu-power-manager_1.0.0-1_amd64.deb
sudo dpkg -i cpu-power-manager_1.0.0-1_amd64.deb
sudo apt-get install -f   # fix dependencies if needed

### Option 2: Build and Install Manually (From Source)
git clone https://github.com/globalcve/cpu-power-manager.git
cd cpu-power-manager
./build.sh
sudo ./install.sh

### Option 3: Build Your Own `.deb`
run option 2 before this.

cargo install cargo-deb
cd cpu-power-manager
cargo deb
sudo dpkg -i target/debian/cpu-power-manager_*.deb

## First Run
Launch from application menu or run:
cpu-power-manager
Grant PolicyKit permissions when prompted. The dashboard will display current CPU information, real-time frequency, temperature, and active governor.

## Quick Actions
Apply a Profile:
- Performance: Maximum CPU performance
- Balanced: Good balance of power and performance
- Power Saver: Extended battery life
- Silent: Quiet operation

Command Line Usage:
cpu-power-manager status
cpu-power-manager set-governor performance
cpu-power-manager set-turbo true
cpu-power-manager apply-profile balanced

## Configuration
Edit `~/.config/cpu-power-manager/config.toml` to customize:
- Auto-start behavior
- Temperature thresholds
- Auto-tuning settings
- Monitoring options

## Troubleshooting
"Permission denied" errors:
sudo apt install policykit-1
Ensure your user is in the `sudo` or `wheel` group.

CPU frequency not changing:
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_driver
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
echo performance | sudo tee /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor

Application won't start:
pkg-config --modversion gtk4
cpu-power-manager --debug
Check logs: `~/.local/share/cpu-power-manager/app.log`

## Support
GitHub Issues: https://github.com/globalcve/cpu-power-manager/issues
