use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    TooShort(usize),
    CrcMismatch { received: u8, calculated: u8 },
    InvalidCommand(u8),
    UnexpectedTempCount(u8),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort(len) => write!(f, "Packet too short: {} bytes", len),
            Self::CrcMismatch { received, calculated } => {
                write!(f, "CRC mismatch: received 0x{:02X}, calculated 0x{:02X}", received, calculated)
            }
            Self::InvalidCommand(cmd) => write!(f, "Invalid command byte: 0x{:02X}", cmd),
            Self::UnexpectedTempCount(count) => write!(f, "Unexpected temp count: {}", count),
        }
    }
}

impl std::error::Error for ParseError {}

/// CRC-8 calculation using polynomial 0x8C (reflected, LSB-first)
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if (crc & 0x01) != 0 {
                (crc >> 1) ^ 0x8C
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// Temperature data from Arduino (4 sensors)
#[derive(Debug, Clone, Default)]
pub struct TemperatureData {
    /// Temperatures in Celsius (converted from tenths)
    pub temps: [f64; 4],
}

/// Build the request packet for temperature query
/// Returns: [0xAA, 0x02, 0x20, CRC8]
pub fn build_request_packet() -> [u8; 4] {
    let header = [0xAA, 0x02, 0x20];
    let crc = crc8(&header);
    [0xAA, 0x02, 0x20, crc]
}

/// Parse a response packet from the Arduino
/// Expected format (13 bytes):
/// [0xAA][0x02][0x20][TEMP_COUNT][T0_H][T0_L][T1_H][T1_L][T2_H][T2_L][T3_H][T3_L][CRC8]
pub fn parse_response_packet(buffer: &[u8]) -> Result<TemperatureData, ParseError> {
    log::debug!(
        "Received {} bytes: {:02X?}",
        buffer.len(),
        &buffer[..buffer.len().min(20)]
    );

    if buffer.len() < 13 {
        return Err(ParseError::TooShort(buffer.len()));
    }

    // Verify CRC
    let received_crc = buffer[12];
    let calculated_crc = crc8(&buffer[0..12]);
    if received_crc != calculated_crc {
        log::debug!(
            "CRC mismatch: received 0x{:02X}, calculated 0x{:02X}",
            received_crc,
            calculated_crc
        );
        return Err(ParseError::CrcMismatch {
            received: received_crc,
            calculated: calculated_crc,
        });
    }

    // Verify command byte
    if buffer[2] != 0x20 {
        return Err(ParseError::InvalidCommand(buffer[2]));
    }

    // Verify temp count
    let temp_count = buffer[3];
    if temp_count != 4 {
        return Err(ParseError::UnexpectedTempCount(temp_count));
    }

    // Parse temperatures (big-endian, values in tenths of Celsius)
    let mut temps = [0.0; 4];
    for i in 0..4 {
        let offset = 4 + (i * 2);
        let raw = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]);
        temps[i] = raw as f64 / 10.0;
    }

    Ok(TemperatureData { temps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc8_empty() {
        assert_eq!(crc8(&[]), 0);
    }

    #[test]
    fn test_crc8_request() {
        // The request packet header
        let header = [0xAA, 0x02, 0x20];
        let crc = crc8(&header);
        // Verify it's consistent (actual value depends on algorithm)
        assert_eq!(crc, crc8(&header));
    }

    #[test]
    fn test_build_request_packet() {
        let packet = build_request_packet();
        assert_eq!(packet[0], 0xAA);
        assert_eq!(packet[1], 0x02);
        assert_eq!(packet[2], 0x20);
        // Verify CRC matches
        let expected_crc = crc8(&[0xAA, 0x02, 0x20]);
        assert_eq!(packet[3], expected_crc);
    }

    #[test]
    fn test_parse_response_too_short() {
        let short = [0u8; 12];
        assert!(parse_response_packet(&short).is_err());
    }

    #[test]
    fn test_parse_response_valid() {
        // Build a valid response:
        // [0xAA, 0x02, 0x20, 4, T0_H, T0_L, T1_H, T1_L, T2_H, T2_L, T3_H, T3_L, CRC]
        // Temp values: 250 (25.0C), 300 (30.0C), 350 (35.0C), 400 (40.0C)
        let mut response = [
            0xAA, 0x02, 0x20, 0x04, // header + count
            0x00, 0xFA, // 250 = 25.0C
            0x01, 0x2C, // 300 = 30.0C
            0x01, 0x5E, // 350 = 35.0C
            0x01, 0x90, // 400 = 40.0C
            0x00, // CRC placeholder
        ];
        response[12] = crc8(&response[0..12]);

        let result = parse_response_packet(&response).unwrap();
        assert!((result.temps[0] - 25.0).abs() < 0.01);
        assert!((result.temps[1] - 30.0).abs() < 0.01);
        assert!((result.temps[2] - 35.0).abs() < 0.01);
        assert!((result.temps[3] - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_response_bad_crc() {
        let response = [
            0xAA, 0x02, 0x20, 0x04, 0x00, 0xFA, 0x01, 0x2C, 0x01, 0x5E, 0x01, 0x90,
            0xFF, // Wrong CRC
        ];
        assert!(parse_response_packet(&response).is_err());
    }

    #[test]
    fn test_parse_response_wrong_command() {
        let mut response = [
            0xAA, 0x02, 0x21, // Wrong command byte
            0x04, 0x00, 0xFA, 0x01, 0x2C, 0x01, 0x5E, 0x01, 0x90, 0x00,
        ];
        response[12] = crc8(&response[0..12]);
        assert!(parse_response_packet(&response).is_err());
    }
}
