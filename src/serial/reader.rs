use crate::serial::{TemperatureData, build_request_packet, parse_response_packet};
use crate::state::TemperatureState;
use log::{debug, error, info, warn};
use serialport::SerialPort;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const READ_TIMEOUT_MS: u64 = 2000;
const RESET_DELAY_MS: u64 = 2000;
const POLL_INTERVAL_SECS: u64 = 10;
const RECONNECT_DELAY_SECS: u64 = 5;
const READ_DELAY_MS: u64 = 100;

pub struct SerialReaderHandle {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl SerialReaderHandle {
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SerialReaderHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct SerialReader {
    device: String,
    baud_rate: u32,
    state: TemperatureState,
}

impl SerialReader {
    pub fn new(device: String, baud_rate: u32, state: TemperatureState) -> Self {
        Self {
            device,
            baud_rate,
            state,
        }
    }

    pub fn spawn(self) -> SerialReaderHandle {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let thread = thread::spawn(move || {
            self.run(running_clone);
        });

        SerialReaderHandle {
            running,
            thread: Some(thread),
        }
    }

    fn run(self, running: Arc<AtomicBool>) {
        while running.load(Ordering::Relaxed) {
            match self.connect() {
                Ok(mut port) => {
                    info!("Connected to {}", self.device);
                    self.state.set_connected(true);

                    while running.load(Ordering::Relaxed) {
                        match self.poll_temperatures(&mut port) {
                            Ok(data) => {
                                debug!(
                                    "Temperatures: {:.1}C, {:.1}C, {:.1}C, {:.1}C",
                                    data.temps[0], data.temps[1], data.temps[2], data.temps[3]
                                );
                                self.state.update(data);
                            }
                            Err(e) => {
                                warn!("Poll error: {}", e);
                                break;
                            }
                        }

                        // Wait for poll interval (interruptible)
                        for _ in 0..POLL_INTERVAL_SECS {
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }
                            thread::sleep(Duration::from_secs(1));
                        }
                    }
                }
                Err(e) => {
                    error!("Connection error: {}", e);
                    self.state.set_connected(false);
                }
            }

            // Wait before reconnect attempt
            if running.load(Ordering::Relaxed) {
                info!("Reconnecting in {} seconds...", RECONNECT_DELAY_SECS);
                for _ in 0..RECONNECT_DELAY_SECS {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        self.state.set_connected(false);
        info!("Serial reader stopped");
    }

    fn connect(&self) -> Result<Box<dyn SerialPort>, String> {
        let mut port = serialport::new(&self.device, self.baud_rate)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(Duration::from_millis(READ_TIMEOUT_MS))
            .open()
            .map_err(|e| format!("Failed to open {}: {}", self.device, e))?;

        // Wait for device reset and startup message
        thread::sleep(Duration::from_millis(RESET_DELAY_MS));

        // Flush any startup messages from the Arduino
        self.flush_input(&mut port);

        Ok(port)
    }

    fn flush_input(&self, port: &mut Box<dyn SerialPort>) {
        let mut buffer = [0u8; 256];
        // Read and discard any pending data (with short timeout)
        loop {
            match port.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    debug!("Flushed {} bytes: {:02X?}", n, &buffer[..n.min(32)]);
                }
                Err(_) => break, // Timeout or error, buffer is empty
            }
        }
    }

    fn poll_temperatures(&self, port: &mut Box<dyn SerialPort>) -> Result<TemperatureData, String> {
        let request = build_request_packet();
        debug!("Sending request: {:02X?}", request);

        port.write_all(&request)
            .map_err(|e| format!("Write error: {}", e))?;

        // Short delay before reading
        thread::sleep(Duration::from_millis(READ_DELAY_MS));

        let mut buffer = [0u8; 256];
        let len = port
            .read(&mut buffer)
            .map_err(|e| format!("Read error: {}", e))?;

        if len == 0 {
            return Err("No data received".to_string());
        }

        parse_response_packet(&buffer[..len]).map_err(|e| e.to_string())
    }
}
