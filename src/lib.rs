#![allow(unused)]
use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

const HEADER_INPUT: u8 = 0xf0;
const HEADER_OUTPUT: u8 = 0xf1;

const CMD_GET: u8 = 0xa1;
const CMD_XXX_176: u8 = 0xb0;
const CMD_SET: u8 = 0xb1;
const CMD_XXX_192: u8 = 0xc0;
const CMD_XXX_193: u8 = 0xc1;

// float
const VOLTAGE_SET: u8 = 193;
const CURRENT_SET: u8 = 194;

const GROUP1_VOLTAGE_SET: u8 = 197;
const GROUP1_CURRENT_SET: u8 = 198;
const GROUP2_VOLTAGE_SET: u8 = 199;
const GROUP2_CURRENT_SET: u8 = 200;
const GROUP3_VOLTAGE_SET: u8 = 201;
const GROUP3_CURRENT_SET: u8 = 202;
const GROUP4_VOLTAGE_SET: u8 = 203;
const GROUP4_CURRENT_SET: u8 = 204;
const GROUP5_VOLTAGE_SET: u8 = 205;
const GROUP5_CURRENT_SET: u8 = 206;
const GROUP6_VOLTAGE_SET: u8 = 207;
const GROUP6_CURRENT_SET: u8 = 208;

const OVP: u8 = 209;
const OCP: u8 = 210;
const OPP: u8 = 211;
const OTP: u8 = 212;
const LVP: u8 = 213;

const METERING_ENABLE: u8 = 216;
const OUTPUT_ENABLE: u8 = 219;

// byte
const BRIGHTNESS: u8 = 214;
const VOLUME: u8 = 215;

const MODEL_NAME: u8 = 222;
const HARDWARE_VERSION: u8 = 223;
const FIRMWARE_VERSION: u8 = 224;
const ALL: u8 = 255;

const PROTECTION_STATES: [&'static str; 7] = ["", "OVP", "OCP", "OPP", "OTP", "LVP", "REP"];

pub struct DPS150 {
    port: SerialStream,
    buffer: Vec<u8>,
    pub set_voltage: f32,
    pub set_current: f32,
    pub input_voltage: f32,
    pub output_voltage: f32,
    pub output_current: f32,
    pub output_power: f32,
    pub output_closed: bool,
    pub upperlimit_voltage: f32,
    pub upperlimit_current: f32,
    pub temperature: f32,
}

impl DPS150 {
    pub fn new(name: &str) -> Result<Self> {
        Ok(Self {
            port: tokio_serial::new(name, 115200)
                .data_bits(tokio_serial::DataBits::Eight)
                .parity(tokio_serial::Parity::None)
                .stop_bits(tokio_serial::StopBits::One)
                .flow_control(tokio_serial::FlowControl::None)
                .open_native_async()?,
            buffer: vec![],
            set_current: 0_f32,
            set_voltage: 0_f32,
            input_voltage: 0_f32,
            output_voltage: 0_f32,
            output_current: 0_f32,
            output_power: 0_f32,
            output_closed: false,
            upperlimit_voltage: 0_f32,
            upperlimit_current: 0_f32,
            temperature: 0_f32,
        })
    }
    pub async fn init_command(&mut self) {
        // CMD_1
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_XXX_193, 0, 1])
            .await;
        // new int[5] { 9600, 19200, 38400, 57600, 115200 }; index + 1
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_XXX_176, 0, 4 + 1])
            .await;
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_GET, MODEL_NAME, 0])
            .await;
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_GET, HARDWARE_VERSION, 0])
            .await;
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_GET, FIRMWARE_VERSION, 0])
            .await;
        self.get_all().await;
    }
    fn get_f32(data: &[u8]) -> f32 {
        unsafe { std::mem::transmute_copy(&*(&data[0] as *const _ as *const [u8; 4])) }
    }
    async fn send_cmd_onebyte(&mut self, data: &[u8]) {
        let mut sum_calc: u8 = 0;
        let mut frame = [0_u8; 6];
        frame[0] = data[0];
        frame[1] = data[1];
        frame[2] = data[2];
        frame[3] = 1;
        frame[4] = data[3];
        for d in frame[2..=4].iter() {
            sum_calc = sum_calc.wrapping_add(*d);
        }
        frame[5] = sum_calc;
        self.port.write(&frame).await.unwrap();
    }
    async fn set_byte_value(&mut self, cmd: u8, val: u8) {
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_SET, cmd, val])
            .await;
    }
    pub async fn get_all(&mut self) {
        self.send_cmd_onebyte(&[HEADER_OUTPUT, CMD_GET, ALL, 0])
            .await;
    }
    pub async fn enable(&mut self) {
        self.set_byte_value(OUTPUT_ENABLE, 1).await;
    }
    pub async fn disable(&mut self) {
        self.set_byte_value(OUTPUT_ENABLE, 0).await;
    }
    pub fn print(&self) {
        println!(
            r#"set_current:{:.2}
set_voltage:{:.2}
input_voltage:{:.2}
output_voltage:{:.2}
output_current:{:.2}
output_power:{:.2}
output_closed:{}
upperlimit_voltage:{:.2}
upperlimit_current:{:.2}
temperature:{:.2}
        "#,
            self.set_current,
            self.set_voltage,
            self.input_voltage,
            self.output_voltage,
            self.output_current,
            self.output_power,
            self.output_closed,
            self.upperlimit_voltage,
            self.upperlimit_current,
            self.temperature
        );
    }
    pub async fn poll(&mut self) -> bool {
        let mut buf = [0_u8; 4096];
        let mut updated = false;
        if let Ok(len) = self.port.read(buf.as_mut_slice()).await {
            println!("uart recv:{:X?}", &buf[..len]);
            if self.parse(&buf[..len]) {
                updated = true;
            }
        }
        updated
    }
    fn parse(&mut self, data: &[u8]) -> bool {
        let mut updated = false;
        self.buffer.extend_from_slice(data);
        let mut pos: usize = 0;
        while self.buffer.len() >= pos + 6 {
            if self.buffer[pos] == HEADER_INPUT && self.buffer[pos + 1] == CMD_GET {
                let length = self.buffer[pos + 3] as usize;
                if pos + 5 + length > self.buffer.len() {
                    break;
                }
                let sum_data = self.buffer[pos + 4 + length];
                let mut sum_calc: u8 = 0;
                for d in self.buffer[pos + 2..pos + length + 4].iter() {
                    sum_calc = sum_calc.wrapping_add(*d);
                }
                if sum_calc != sum_data {
                    println!("invalid data sum, need:{:X} but:{:X}", sum_calc, sum_data);
                    pos += 1;
                } else {
                    let frame_data = &self.buffer[pos + 4..pos + 4 + length];
                    match self.buffer[pos + 2] {
                        192 => {
                            // input voltage
                            self.input_voltage = Self::get_f32(&frame_data[0..]);
                        }
                        195 => {
                            // output voltage, current, power
                            self.output_voltage = Self::get_f32(&frame_data[0..]);
                            self.output_current = Self::get_f32(&frame_data[4..]);
                            self.output_power = Self::get_f32(&frame_data[8..]);
                        }
                        196 => {
                            // temperature
                            self.temperature = Self::get_f32(&frame_data[0..]);
                        }
                        219 => {
                            // output closed?
                            self.output_closed = frame_data[0] == 1;
                        }
                        222 => {
                            println!(
                                "model name:{}",
                                String::from_utf8(frame_data.to_vec()).unwrap()
                            );
                        }
                        223 => {
                            println!(
                                "hardware version:{}",
                                String::from_utf8(frame_data.to_vec()).unwrap()
                            );
                        }
                        224 => {
                            println!(
                                "firmware version:{}",
                                String::from_utf8(frame_data.to_vec()).unwrap()
                            );
                        }
                        226 => {
                            self.upperlimit_voltage = Self::get_f32(&frame_data[0..]);
                        }
                        227 => {
                            self.upperlimit_current = Self::get_f32(&frame_data[0..]);
                        }
                        255 => {
                            // set all
                            self.input_voltage = Self::get_f32(&frame_data[0..]);
                            self.set_voltage = Self::get_f32(&frame_data[4..]);
                            self.set_current = Self::get_f32(&frame_data[8..]);
                            self.output_voltage = Self::get_f32(&frame_data[12..]);
                            self.output_current = Self::get_f32(&frame_data[16..]);
                            self.output_power = Self::get_f32(&frame_data[20..]);
                            self.temperature = Self::get_f32(&frame_data[24..]);
                            self.output_closed = frame_data[107] == 1;
                            self.upperlimit_voltage = Self::get_f32(&frame_data[111..]);
                            self.upperlimit_voltage = Self::get_f32(&frame_data[115..]);
                        }

                        _ => {
                            println!("ignored cmd:{:X}", self.buffer[pos + 2])
                        }
                    }
                    pos = pos + 5 + length;
                    updated = true;
                }
            } else {
                pos += 1;
            }
        }
        self.buffer.drain(0..pos);
        updated
    }
}
