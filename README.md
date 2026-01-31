# cc-ardutemp

A [CoolerControl](https://gitlab.com/coolercontrol/coolercontrol) device service plugin for Arduino temperature sensors via serial connection.

This plugin reads temperature data from an Arduino connected via USB serial port and exposes the sensors to CoolerControl.

## Requirements

- CoolerControl 1.4.0 or newer
- Arduino with temperature sensors sending data via serial (see [Arduino Firmware](#arduino-firmware))
- USB serial connection (typically `/dev/ttyUSB0` or `/dev/ttyACM0`)

## Installation

### Quick Install

Run the install script which will guide you through device selection:

```bash
curl -fsSL https://raw.githubusercontent.com/RadiatorTwo/cc-ardutemp/master/install.sh -o /tmp/install.sh && bash /tmp/install.sh
```

Or install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/RadiatorTwo/cc-ardutemp/master/install.sh -o /tmp/install.sh && bash /tmp/install.sh v0.1.0
```

### From Source

Requirements:
- Rust 1.86.0 or newer
- make
- cargo
- protobuf-compiler

```bash
git clone https://github.com/RadiatorTwo/cc-ardutemp.git
cd cc-ardutemp
make install
```

After installation, configure the device path in `/etc/coolercontrol/plugins/ardu-temp-bridge/manifest.toml`.

## Configuration

The plugin requires configuration of your serial device. Edit the manifest file:

```bash
sudo nano /etc/coolercontrol/plugins/ardu-temp-bridge/manifest.toml
```

Adjust the `args` line to match your device:

```toml
args = "--device /dev/ttyUSB0 --baud 57600"
```

Common device paths:
- `/dev/ttyUSB0` - USB-to-Serial adapters (FTDI, CH340, etc.)
- `/dev/ttyACM0` - Arduino with native USB (Leonardo, Micro, Due, etc.)

### Options

| Argument   | Environment Variable | Default         | Description              |
|------------|---------------------|-----------------|--------------------------|
| `--device` | `ARDU_DEVICE`       | `/dev/ttyACM0`  | Serial port device path  |
| `--baud`   | `ARDU_BAUD`         | `57600`         | Serial port baud rate    |
| `--debug`  | -                   | `false`         | Enable debug logging     |

## Post-Installation

Restart the CoolerControl daemon to load the plugin:

```bash
sudo systemctl restart coolercontrold
```

## Troubleshooting

View plugin logs:

```bash
journalctl -u coolercontrold -f | grep ardu-temp-bridge
```

### Permission Issues

If the plugin cannot access the serial port, ensure the service has proper permissions. The `privileged = true` setting in the manifest allows the plugin to access serial devices.

### Device Not Found

List available serial devices:

```bash
ls -la /dev/ttyUSB* /dev/ttyACM* 2>/dev/null
```

Check which device your Arduino is using:

```bash
dmesg | grep tty
```

## Arduino Firmware

The Arduino should send temperature readings via serial in the following format:

```
TEMP:sensor_name:value_in_millidegrees
```

Example: `TEMP:CPU:45000` for 45.0°C

## Uninstall

```bash
sudo rm -rf /etc/coolercontrol/plugins/ardu-temp-bridge
sudo systemctl restart coolercontrold
```

Or using make:

```bash
make uninstall
```

## License

GPL-3.0-or-later
