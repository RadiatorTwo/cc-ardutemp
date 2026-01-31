#!/usr/bin/env bash
set -e

# Configuration
REPO_URL="https://github.com/RadiatorTwo/cc-ardutemp"
RAW_URL="https://raw.githubusercontent.com/RadiatorTwo/cc-ardutemp"
PLUGIN_DIR="/etc/coolercontrol/plugins"
SERVICE_ID="ardu-temp-bridge"
EXECUTABLE="ardu-temp-bridge"
DEFAULT_BAUD="57600"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Cleanup on exit
cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT

# Check requirements
check_requirements() {
    for cmd in curl; do
        if ! command -v "$cmd" &> /dev/null; then
            error "$cmd is required but not installed."
        fi
    done
}

# Detect architecture
detect_arch() {
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64)
            ARCH="x86_64"
            ;;
        *)
            error "Unsupported architecture: $ARCH (only x86_64 is supported)"
            ;;
    esac
    info "Detected architecture: $ARCH"
}

# Get latest version from GitHub
get_latest_version() {
    info "Fetching latest version..."
    LATEST=$(curl -fsSL "https://api.github.com/repos/RadiatorTwo/cc-ardutemp/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || echo "")
    if [ -z "$LATEST" ]; then
        warn "Could not fetch latest version, using main branch"
        VERSION="main"
    else
        VERSION="$LATEST"
    fi
    info "Using version: $VERSION"
}

# List available serial devices
list_devices() {
    echo ""
    echo -e "${BLUE}Available serial devices:${NC}"
    echo ""

    DEVICES=()
    i=1

    # Find ttyUSB and ttyACM devices
    shopt -s nullglob
    for dev in /dev/ttyUSB* /dev/ttyACM*; do
        DEVICES+=("$dev")
        # Try to get device info
        DEVINFO=""
        if [ -r "/sys/class/tty/$(basename "$dev")/device/interface" ]; then
            DEVINFO=$(cat "/sys/class/tty/$(basename "$dev")/device/interface" 2>/dev/null || echo "")
        fi
        if [ -z "$DEVINFO" ] && [ -r "/sys/class/tty/$(basename "$dev")/device/../product" ]; then
            DEVINFO=$(cat "/sys/class/tty/$(basename "$dev")/device/../product" 2>/dev/null || echo "")
        fi
        if [ -n "$DEVINFO" ]; then
            echo -e "  ${GREEN}$i)${NC} $dev - $DEVINFO"
        else
            echo -e "  ${GREEN}$i)${NC} $dev"
        fi
        ((i++))
    done
    shopt -u nullglob

    if [ ${#DEVICES[@]} -eq 0 ]; then
        warn "No serial devices found!"
        echo ""
        echo "Please connect your Arduino and try again."
        echo "You can also manually specify the device path after installation"
        echo "by editing: $PLUGIN_DIR/$SERVICE_ID/manifest.toml"
        echo ""
        DEVICES+=("/dev/ttyUSB0")
        DEVICES+=("/dev/ttyACM0")
        echo -e "  ${GREEN}1)${NC} /dev/ttyUSB0 (default for USB-to-Serial adapters)"
        echo -e "  ${GREEN}2)${NC} /dev/ttyACM0 (default for Arduino with native USB)"
    fi

    DEVICES+=("custom")
    echo -e "  ${GREEN}$i)${NC} Enter custom device path"
    echo ""
}

# Select device
select_device() {
    list_devices

    while true; do
        read -p "Select device [1-${#DEVICES[@]}]: " choice

        if [[ "$choice" =~ ^[0-9]+$ ]] && [ "$choice" -ge 1 ] && [ "$choice" -le ${#DEVICES[@]} ]; then
            idx=$((choice - 1))
            if [ "${DEVICES[$idx]}" = "custom" ]; then
                read -p "Enter device path: " SELECTED_DEVICE
                if [ -z "$SELECTED_DEVICE" ]; then
                    warn "No device path entered, using /dev/ttyUSB0"
                    SELECTED_DEVICE="/dev/ttyUSB0"
                fi
            else
                SELECTED_DEVICE="${DEVICES[$idx]}"
            fi
            break
        else
            warn "Invalid selection. Please enter a number between 1 and ${#DEVICES[@]}"
        fi
    done

    info "Selected device: $SELECTED_DEVICE"
    echo ""

    # Ask for baud rate
    read -p "Enter baud rate [$DEFAULT_BAUD]: " BAUD_RATE
    BAUD_RATE="${BAUD_RATE:-$DEFAULT_BAUD}"
    info "Using baud rate: $BAUD_RATE"
}

# Download files
download_files() {
    TEMP_DIR=$(mktemp -d)
    info "Downloading files to $TEMP_DIR..."

    # Download binary
    BINARY_URL="$REPO_URL/releases/download/$VERSION/${EXECUTABLE}-${ARCH}"
    if [ "$VERSION" = "main" ]; then
        error "No releases available yet. Please build from source using 'make install'"
    fi

    info "Downloading binary from $BINARY_URL..."
    if ! curl -fsSL "$BINARY_URL" -o "$TEMP_DIR/$EXECUTABLE"; then
        error "Failed to download binary. Please check if the release exists."
    fi

    # Download manifest template
    info "Downloading manifest..."
    if ! curl -fsSL "$RAW_URL/$VERSION/manifest.toml" -o "$TEMP_DIR/manifest.toml"; then
        # Create manifest if download fails
        cat > "$TEMP_DIR/manifest.toml" << EOF
# CoolerControl Service Plugin Manifest

id = "$SERVICE_ID"
type = "device"
description = "Arduino Temperature Sensors via Serial"
version = "${VERSION#v}"
executable = "$EXECUTABLE"
args = "--device $SELECTED_DEVICE --baud $BAUD_RATE"
privileged = true
EOF
    else
        # Update manifest with selected device
        sed -i "s|--device [^ ]*|--device $SELECTED_DEVICE|g" "$TEMP_DIR/manifest.toml"
        sed -i "s|--baud [0-9]*|--baud $BAUD_RATE|g" "$TEMP_DIR/manifest.toml"
    fi
}

# Install files
install_files() {
    info "Installing to $PLUGIN_DIR/$SERVICE_ID..."

    sudo mkdir -p "$PLUGIN_DIR/$SERVICE_ID"

    # Backup existing manifest if present
    if [ -f "$PLUGIN_DIR/$SERVICE_ID/manifest.toml" ]; then
        warn "Backing up existing manifest to manifest.toml.bak"
        sudo cp "$PLUGIN_DIR/$SERVICE_ID/manifest.toml" "$PLUGIN_DIR/$SERVICE_ID/manifest.toml.bak"
    fi

    sudo install -m755 "$TEMP_DIR/$EXECUTABLE" "$PLUGIN_DIR/$SERVICE_ID/"
    sudo install -m644 "$TEMP_DIR/manifest.toml" "$PLUGIN_DIR/$SERVICE_ID/"

    info "Installation complete!"
}

# Main
main() {
    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║${NC}     CoolerControl Arduino Temperature Bridge Installer      ${GREEN}║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    check_requirements
    detect_arch

    # Get version from argument or fetch latest
    if [ -n "$1" ]; then
        VERSION="$1"
        info "Using specified version: $VERSION"
    else
        get_latest_version
    fi

    select_device
    download_files
    install_files

    echo ""
    echo -e "${GREEN}════════════════════════════════════════════════════════════════${NC}"
    echo ""
    info "Plugin installed successfully!"
    echo ""
    echo "Configuration:"
    echo "  Device: $SELECTED_DEVICE"
    echo "  Baud:   $BAUD_RATE"
    echo ""
    echo "To change settings, edit:"
    echo "  $PLUGIN_DIR/$SERVICE_ID/manifest.toml"
    echo ""
    echo -e "${YELLOW}Don't forget to restart the CoolerControl daemon:${NC}"
    echo "  sudo systemctl restart coolercontrold"
    echo ""
}

main "$@"
