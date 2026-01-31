use crate::device_service::v1::device_service_server::DeviceService;
use crate::device_service::v1::{
    CustomFunctionOneRequest, CustomFunctionOneResponse, EnableManualFanControlRequest,
    EnableManualFanControlResponse, FixedDutyRequest, FixedDutyResponse, HealthRequest,
    HealthResponse, InitializeDeviceRequest, InitializeDeviceResponse, LcdRequest, LcdResponse,
    LightingRequest, LightingResponse, ListDevicesRequest, ListDevicesResponse,
    ResetChannelRequest, ResetChannelResponse, ShutdownRequest, ShutdownResponse,
    SpeedProfileRequest, SpeedProfileResponse, StatusRequest, StatusResponse, health_response,
};
use crate::models::v1::{Device, DeviceInfo, TempInfo};
use crate::state::TemperatureState;
use crate::{SERVICE_ID, VERSION};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tonic::{Request, Response, Status};

const DEVICE_ID: &str = "arduino-temp";
const DEVICE_NAME: &str = "Arduino Temp";

pub struct ArduTempService {
    state: TemperatureState,
    start_time: Instant,
    uptime: AtomicU64,
}

impl ArduTempService {
    pub fn new(state: TemperatureState) -> Self {
        Self {
            state,
            start_time: Instant::now(),
            uptime: AtomicU64::new(0),
        }
    }

    fn update_uptime(&self) -> u64 {
        let uptime = self.start_time.elapsed().as_secs();
        self.uptime.store(uptime, Ordering::Relaxed);
        uptime
    }

    fn build_device(&self) -> Device {
        let mut temps = HashMap::new();
        for i in 1..=4 {
            temps.insert(
                format!("temp{}", i),
                TempInfo {
                    label: format!("Arduino Temp {}", i),
                    number: i,
                },
            );
        }

        Device {
            id: DEVICE_ID.to_string(),
            name: DEVICE_NAME.to_string(),
            uid_info: None,
            info: Some(DeviceInfo {
                channels: HashMap::new(),
                temps,
                lighting_speeds: vec![],
                temp_min: Some(0.0),
                temp_max: Some(100.0),
                profile_min_length: None,
                profile_max_length: None,
                model: Some("Arduino Temperature Sensor Bridge".to_string()),
                driver_info: None,
            }),
        }
    }
}

#[tonic::async_trait]
impl DeviceService for ArduTempService {
    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let status = if self.state.is_connected() {
            health_response::Status::Ok
        } else {
            health_response::Status::Warning
        };

        let reply = HealthResponse {
            name: SERVICE_ID.to_string(),
            version: VERSION.to_string(),
            status: status.into(),
            uptime_seconds: self.update_uptime(),
        };
        Ok(Response::new(reply))
    }

    async fn list_devices(
        &self,
        _request: Request<ListDevicesRequest>,
    ) -> Result<Response<ListDevicesResponse>, Status> {
        Ok(Response::new(ListDevicesResponse {
            devices: vec![self.build_device()],
        }))
    }

    async fn initialize_device(
        &self,
        _request: Request<InitializeDeviceRequest>,
    ) -> Result<Response<InitializeDeviceResponse>, Status> {
        Ok(Response::new(InitializeDeviceResponse {}))
    }

    async fn shutdown(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        Ok(Response::new(ShutdownResponse {}))
    }

    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        if request.get_ref().device_id != DEVICE_ID {
            return Ok(Response::new(StatusResponse { status: vec![] }));
        }

        let temps = self.state.get_temperatures();
        let status: Vec<_> = temps
            .iter()
            .enumerate()
            .map(|(i, &temp)| crate::models::v1::Status {
                id: format!("temp{}", i + 1),
                metric: Some(crate::models::v1::status::Metric::Temp(temp)),
            })
            .collect();

        Ok(Response::new(StatusResponse { status }))
    }

    async fn reset_channel(
        &self,
        _request: Request<ResetChannelRequest>,
    ) -> Result<Response<ResetChannelResponse>, Status> {
        Ok(Response::new(ResetChannelResponse {}))
    }

    async fn enable_manual_fan_control(
        &self,
        _request: Request<EnableManualFanControlRequest>,
    ) -> Result<Response<EnableManualFanControlResponse>, Status> {
        Err(Status::unimplemented("No fans available"))
    }

    async fn fixed_duty(
        &self,
        _request: Request<FixedDutyRequest>,
    ) -> Result<Response<FixedDutyResponse>, Status> {
        Err(Status::unimplemented("No fans available"))
    }

    async fn speed_profile(
        &self,
        _request: Request<SpeedProfileRequest>,
    ) -> Result<Response<SpeedProfileResponse>, Status> {
        Err(Status::unimplemented("No firmware profiles"))
    }

    async fn lighting(
        &self,
        _request: Request<LightingRequest>,
    ) -> Result<Response<LightingResponse>, Status> {
        Err(Status::unimplemented("No lighting channels"))
    }

    async fn lcd(&self, _request: Request<LcdRequest>) -> Result<Response<LcdResponse>, Status> {
        Err(Status::unimplemented("No LCD channels"))
    }

    async fn custom_function_one(
        &self,
        _request: Request<CustomFunctionOneRequest>,
    ) -> Result<Response<CustomFunctionOneResponse>, Status> {
        Err(Status::unimplemented("No custom functions"))
    }
}
