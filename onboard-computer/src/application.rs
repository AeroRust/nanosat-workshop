use core::fmt::Write as _;

use bmp388::BMP388;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Executor;
use embassy_futures::join::join;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, CriticalSectionMutex},
    mutex::Mutex,
};
use embassy_time::{Duration, Timer};

use esp_println::println;

use hal::{
    clock::{ClockControl, Clocks},
    embassy,
    gpio::{Gpio7, Output, PushPull},
    i2c::I2C,
    interrupt::{self, Priority},
    peripherals::{Interrupt, Peripherals, I2C0, UART1, USB_DEVICE},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart,
    Delay,
    // otg_fs::{UsbBus, USB},
    Rng,
    Rtc,
    Uart,
    UsbSerialJtag,
    IO,
};

use embedded_io_async::{Read, Write};

use icm42670::accelerometer::Accelerometer;

use nmea::ParseResult;
use static_cell::make_static;

/// The Rust ESP32-C3 board has onboard LED on GPIO 7
pub type OnboardLed = Gpio7<Output<PushPull>>;

static MOCK_SENTENCES: &'static str = include_str!("../../tests/nmea.log");

// Uart rx_fifo_full_threshold
const UART_READ_BUF_SIZE: usize = 64;

// EOT (CTRL-D)
const UART_AT_CMD: u8 = 0x04;

const NMEA_SENTENCE_TERMINATOR: &str = "\r\n";

pub type I2C0AsyncDeviceType = I2cDevice<'static, CriticalSectionRawMutex, I2C<'static, I2C0>>;
pub type I2C0AsyncMutex = Mutex<CriticalSectionRawMutex, I2C<'static, I2C0>>;

pub type I2C0BlockingMutex = critical_section::Mutex<core::cell::RefCell<I2C<'static, I2C0>>>;
pub type I2C0BlockingDeviceType =
    embedded_hal_bus::i2c::CriticalSectionDevice<'static, I2C<'static, I2C0>>;

pub type I2C0Mutex = I2C0AsyncMutex;
pub type I2C0DeviceType = I2C0AsyncDeviceType;

// pub type I2C0Mutex = I2C0BlockingMutex;
// pub type I2C0DeviceType = I2C0BlockingDeviceType;

pub struct Application {
    clocks: Clocks<'static>,
    // TODO: Uncomment when you create a `Uart` instance of the `UART1` peripheral
    uart: Uart<'static, UART1>,
    // TODO: Uncomment when you create a `Rng` instance
    rng: Rng,
    // TODO: Uncomment when you create the `OnboardLed` instance
    onboard_led: OnboardLed,

    // TODO: Uncomment when you create the `UsbSerialJtag` instance
    // usb_serial_jtag: UsbSerialJtag<'static>,
    i2c: &'static I2C0Mutex,
}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init(peripherals: Peripherals) -> Self {
        let system: SystemParts = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        let mut timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
        let mut wdt0 = timer_group0.wdt;
        let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
        let mut wdt1 = timer_group1.wdt;

        // Disable watchdog timers
        rtc.swd.disable();
        rtc.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        #[cfg(feature = "embassy-time-systick")]
        embassy::init(
            &clocks,
            hal::systimer::SystemTimer::new(peripherals.SYSTIMER),
        );

        #[cfg(feature = "embassy-time-timg0")]
        embassy::init(&clocks, timer_group0.timer0);

        // Setup peripherals for application
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // Onboard LED
        // Rust ESP32-C3 schematics: https://raw.githubusercontent.com/esp-rs/esp-rust-board/master/assets/rust_board_v1_pin-layout.png
        // Set GPIO7 as an output, and set its state high initially.
        let mut onboard_led = io.pins.gpio7.into_push_pull_output();
        onboard_led.set_high().unwrap();

        // Setup Random Generator for GNSS Reading
        // Hal example: https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/rng.rs
        let rng = Rng::new(peripherals.RNG);
        // let rng = todo!("Initialize the Random generator");

        // The Rust ESP32-C3 board has debug UART on pins 21 (TX) and 20 (RX)
        // but you should use GPIO pins 0 (TX) and 1 (RX) for sending and receiving data respectively
        // to/from the `power-system` board.
        // TODO: Configure the UART 1 peripheral
        // let mut uart1 = todo!("Configure UART 1 at pins 0 (TX) and 1 (RX) with `None` or default for the `Config`");
        let mut uart1 = Uart::new_with_config(
            peripherals.UART1,
            uart::config::Config {
                // default baudrate
                // https://files.waveshare.com/upload/0/06/Quectel_LC26G%26LC76G%26LC86G_GNSS_Protocol_Specification_V1.0.0_Preliminary.pdf
                baudrate: 115200,
                data_bits: uart::config::DataBits::DataBits8,
                parity: uart::config::Parity::ParityNone,
                stop_bits: uart::config::StopBits::STOP1,
            },
            Some(uart::TxRxPins::new_tx_rx(
                io.pins.gpio0.into_push_pull_output(),
                io.pins.gpio1.into_floating_input(),
            )),
            &clocks,
        );
        // uart1.set_at_cmd(uart::config::AtCmdConfig::new(
        //     None,
        //     None,
        //     None,
        //     UART_AT_CMD,
        //     None,
        // ));
        // uart1
        //     .set_rx_fifo_full_threshold(UART_READ_BUF_SIZE as u16)
        //     .unwrap();
        interrupt::enable(Interrupt::UART1, Priority::Priority1).unwrap();

        // let mut usb_serial_jtag = UsbSerialJtag::new(peripherals.USB_DEVICE);
        // usb_serial_jtag.listen_rx_packet_recv_interrupt();
        // timer_group0.timer0.start(1u64.secs());
        // interrupt::enable(Interrupt::USB_DEVICE, interrupt::Priority::Priority1).unwrap();

        // let usb = USB::new(
        //     peripherals.USB0,
        //     io.pins.gpio18,
        //     io.pins.gpio19,
        //     io.pins.gpio20,
        // );

        // let usb_bus = UsbBus::new(usb, unsafe { &mut EP_MEMORY });

        // let mut serial = usbd_serial::SerialPort::new(&usb_bus);

        // let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        //     .manufacturer("esp-hal")
        //     .product("esp-hal")
        //     .serial_number("12345678")
        //     .device_class(usbd_serial::USB_CLASS_CDC)
        //     .build();

        let i2c0 = I2C::new(
            peripherals.I2C0,
            io.pins.gpio10,
            io.pins.gpio8,
            400_u32.kHz(),
            // clocks.
            &clocks,
        );
        interrupt::enable(Interrupt::I2C_EXT0, interrupt::Priority::Priority1).unwrap();
        println!("Here 1");
        let i2c = make_static!(Mutex::<CriticalSectionRawMutex, _>::new(i2c0));
        // let i2c = make_static!(critical_section::Mutex::new(core::cell::RefCell::new(i2c0)));

        println!("Peripherals initialized");
        Self {
            clocks,
            uart: uart1,
            // usb_serial_jtag,
            rng,
            onboard_led,
            i2c,
        }
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self, executor: &'static mut Executor) -> ! {
        executor.run(|spawner| {
            // spawner.must_spawn(run_uart_plotter(self.uart));
            // spawner.must_spawn(run_gnss_mocked(self.rng));
            // spawner.must_spawn(run_gnss_receive(self.uart));
            // spawner.must_spawn(run_imu(self.i2c));
            // spawner.must_spawn(run_temp_humid(
            //     self.i2c,
            //     hal::Delay::new(&self.clocks),
            // ));
            // spawner.must_spawn(run_usb_serial_jtag(self.usb_serial_jtag));
            // spawner.must_spawn(run_usb_serial(self.usb_serial));
            spawner.must_spawn(run_pressure_sense(self.i2c, embassy_time::Delay));
        })
    }
}

/// # Exercise: Parse GNSS data from NMEA 0183 sentences
///
/// This task parses NMEA sentences simulated from a GNSS data log file
/// The task picks random sentences from a log file and looks out for `GNS` and `GSV` messages
///
///
/// Print the ID's of satellites used for fix in GSA sentence and the satellites in view from the GSV sentence
///
/// `nmea` crate docs: <https://docs.rs/nmea>
///
/// 0. Add the `nmea` crate to the `Cargo.toml` of the `onboard-computer`
/// - You should exclude the `default-features` of the crate, as we operate in `no_std` environment
/// - Alternate solution: You can add the crate to the `workspace` dependencies of the project in the `Cargo.toml` of the project
///   For more details see: <https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table>
///
/// 1. Use the `nmea` crate to parse the sentences
/// 2. Print the parsing result (for debugging purposes) using `esp_println::println!()`
/// 3. Use a match on the result and handle the cases:
/// - GSA - print "The IDs of satellites used for fix: {x:?}" field
/// - GSV - print "Satellites in View: {x}" field
/// 4. Repeat this processes every 2 seconds.
#[embassy_executor::task]
async fn run_gnss_mocked(mut rng: Rng) {
    loop {
        let num = rng.random() as u8;
        let sentence = MOCK_SENTENCES.lines().nth(num as usize).unwrap();

        // println!("(debug) NMEA sentence at line: {num}: {sentence}");
        // 1. Use the `nmea` crate to parse the sentences
        // TODO: Uncomment line and finish the `todo!()`
        // let parse_result = todo!("call nmea::parse_str");
        let parse_result = nmea::parse_str(sentence);
        // 2. Print the parsing result (for debugging purposes) using `esp_println::println!()`
        // println!("{:?}", parse_result);
        // 3. Use a match on the result and handle the sentences:
        // - GSA
        // - GSV
        match parse_result {
            Ok(ParseResult::GSA(gsa_data)) => {
                println!("GSA: Fix satellites: {:?}", gsa_data.fix_sats_prn);
            }
            Ok(ParseResult::GSV(gsv_data)) => {
                println!("GSV: Satellites in view: {}", gsv_data.sats_in_view);
            }
            _ => {}
        }
        Timer::after(Duration::from_secs(2)).await;
    }
}

/// https://www.waveshare.com/wiki/LC76G_GNSS_Module
#[embassy_executor::task]
async fn run_gnss_receive(uart: Uart<'static, UART1>) {
    let (mut tx, mut rx) = uart.split();

    esp_println::println!("GNSS: Uart reading...");
    // max message size to receive
    // leave some extra space for AT-CMD characters
    const MAX_BUFFER_SIZE: usize = 10 * UART_READ_BUF_SIZE + 16;

    let mut rbuf: [u8; MAX_BUFFER_SIZE] = [0_u8; MAX_BUFFER_SIZE];
    let mut offset: usize = 0;
    loop {
        let r = embedded_io_async::Read::read(&mut rx, &mut rbuf).await;
        match r {
            Ok(len) => {
                let ascii_data = core::str::from_utf8(&rbuf[..len]).unwrap();
                esp_println::println!("Read: {len}, data: {}", ascii_data);
            }
            Err(e) => {
                esp_println::println!("RX Error: {:?}", e);
                continue;
            }
        }
    }

    loop {
        let r = embedded_io_async::Read::read(&mut rx, &mut rbuf[offset..]).await;
        match r {
            Ok(len) => {
                offset += len;
                esp_println::println!("Read: {len}");
                // esp_println::println!("Read: {len}, data: {:?}", &rbuf[..offset]);
            }
            Err(e) => {
                esp_println::println!("RX Error: {:?}", e);
                offset = 0;
                continue;
            }
        }

        if offset > 0 {
            let ascii = match core::str::from_utf8(&rbuf[..offset]) {
                Ok(received_str) => received_str,
                Err(err) => {
                    println!("UTF-8 error parsing UART bytes as string: {err}");
                    // clear the offset
                    offset = 0;
                    continue;
                }
            };

            let has_full_end_sentences = ascii.ends_with(NMEA_SENTENCE_TERMINATOR);
            let mut sentences = ascii.split_terminator(['\r', '\n'].as_slice());

            let mut non_full_buf = [0_u8; 200];
            let mut non_full_len = 0_usize;

            {
                use core::iter::DoubleEndedIterator;
                // remove the last non-full sentence
                let non_full_sentence = sentences.next_back().unwrap();
                non_full_len = non_full_sentence.len();
                // fill the the buffer with the non-full sentence
                non_full_buf[..non_full_len].copy_from_slice(non_full_sentence.as_bytes());
            }

            for sentence in sentences {
                parse_sentence(sentence);
            }

            if non_full_len > 0 {
                rbuf[..non_full_len].copy_from_slice(&non_full_buf[..non_full_len]);
                offset = non_full_len
            } else {
                offset = 0;
            }
        }
    }
}

fn parse_sentence(sentence: &str) {
    // 1. Parse the sentences splitting them by `\r\n`

    // for sentence in sentences.split_terminator("\r\n") {
    let parse_result = nmea::parse_str(sentence);

    // 2. Use a match on the result and handle the sentences:
    // - GSA
    // - GSV
    // - RMC
    match parse_result {
        Ok(ParseResult::GSA(gsa_data)) => {
            println!("{gsa_data:?}");
        }
        Ok(ParseResult::GSV(gsv_data)) => {
            println!("{gsv_data:?}");
        }
        Ok(ParseResult::RMC(rmc_data)) => {
            println!("{rmc_data:?}");
        }
        Err(err) => {
            println!("Error: {err:?}; sentence: '{sentence}'");
        }
        _ => {
            // skip
        }
    }
    // }
}

/// # Exercise: Receive battery percentage over UART from the power-system
///
/// 1. create an infinite loop that
/// 2. tries to read the Battery Percentage value sent from the Power System,
/// 3. prints the value on success or the error on failure (using Debug formatting),
/// 4. Repeat the read every 20 milliseconds
#[embassy_executor::task]
async fn run_uart(uart: Uart<'static, UART1>) {
    let (mut tx, mut rx) = uart.split();
    // single byte battery percentage
    // loop {
    //     let mut buf = [0; 256];
    //     match Read::read(&mut uart, &mut buf).await {
    //         Ok(battery_percentage) => {
    //             println!("Battery: {battery_percentage}%");
    //         }
    //         Err(err) => {
    //             println!("Error: {err:?}")
    //         }
    //     }
    //     Timer::after(Duration::from_millis(20)).await;
    // }

    let send = async {
        esp_println::println!("Uart writing...");
        loop {
            let data = "Hello async serial. Enter something ended with EOT (CTRL-D).\r\n";
            // write!(&mut tx, "Hello async serial. Enter something ended with EOT (CTRL-D).\r\n").unwrap();
            // use core::fmt::Write;
            // embedded_io_async::Write::flush(&mut tx).await.unwrap();
            Write::write_all(&mut tx, data.as_bytes()).await.unwrap();
            // match Write::write_all(&mut tx, data.as_bytes()).await {
            //     Ok(_) => println!("wrote '{data}' ({} bytes total) to UART", data.as_bytes().len()),
            //     Err(err) => println!("Error writing to UART: {err:?}"),
            // }
            Timer::after(Duration::from_millis(5000)).await;
        }
    };

    let receive = async {
        esp_println::println!("Uart reading...");
        // max message size to receive
        // leave some extra space for AT-CMD characters
        const MAX_BUFFER_SIZE: usize = 10 * UART_READ_BUF_SIZE + 16;

        let mut rbuf: [u8; MAX_BUFFER_SIZE] = [0_u8; MAX_BUFFER_SIZE];
        let mut offset: usize = 0;
        loop {
            let r = embedded_io_async::Read::read(&mut rx, &mut rbuf[offset..]).await;
            match r {
                Ok(len) => {
                    offset += len;
                    // esp_println::println!("Read: {len}, data: {:?}", &rbuf[..offset]);
                    match core::str::from_utf8(&rbuf) {
                        Ok(received_str) => println!("Received String over UART: {received_str}"),
                        Err(err) => println!("UTF-8 error parsing UART bytes as string: {err}"),
                    }

                    offset = 0;
                }
                Err(e) => esp_println::println!("RX Error: {:?}", e),
            }
        }
    };

    // receive.await
    join(receive, send).await;
}

#[embassy_executor::task]
async fn run_temp_humid(
    i2c: I2C<'static, I2C0>,
    // i2c: &'static I2C0Mutex,
    // i2c: &'static Mutex<CriticalSectionRawMutex, I2C<'static, I2C0>>,
    mut delay: Delay,
) {
    // let i2c_device = I2C0DeviceType::new(i2c);
    // let i2c_device = I2cDevice::new(i2c);
    // let mut i2c = i2c.lock().await;

    // let mut sensor = shtcx::shtc3(i2c_device);
    let mut sensor = shtcx::shtc3(i2c);
    // let mut delay = hal::Delay::new;

    let wait_for_measure_micros =
        shtcx::max_measurement_duration(&sensor, shtcx::PowerMode::NormalMode);
    let wait_for = Duration::from_micros(wait_for_measure_micros.into());

    let measure_every = Duration::from_millis(500) - wait_for;
    loop {
        if let Err(err) = sensor.start_measurement(shtcx::PowerMode::NormalMode) {
            println!("(shtc3::start_measurement) Error: {err:?}");
            continue;
        }

        Timer::after(wait_for).await;
        if let Err(err) = sensor.get_measurement_result() {
            println!("(shtc3::start_measurement) Error: {err:?}");
            continue;
        }
        let combined = match sensor.measure(shtcx::PowerMode::NormalMode, &mut delay) {
            Ok(value) => {
                println!(
                    "Combined: {} °C / {} %RH",
                    value.temperature.as_degrees_celsius(),
                    value.humidity.as_percent()
                );
            }
            Err(err) => {
                println!("(shtc3::start_measure) Error: {err:?}");
                // try again skipping the measure every time.
                continue;
            }
        };

        // println!("Temperature: {} °C", temperature.as_degrees_celsius());
        // println!("Humidity: {} %RH", humidity.as_percent());

        Timer::after(measure_every).await;
    }
}
#[embassy_executor::task]
async fn run_imu(i2c: I2C<'static, I2C0>) {
    Timer::after(Duration::from_millis(1000)).await;

    println!("Initialize IMU Icm 42670...");
    // loop {
    // Address::Primary is address `0x68`
    let mut imu = match icm42670::Icm42670::new(i2c, icm42670::Address::Secondary).await {
        Ok(imu) => imu,
        Err(err) => {
            panic!("Error initializing IMU: {err:?}");
        }
    };

    loop {
        let gyro_norm = imu.gyro_norm_async().await;
        let accelerometer = imu.accel_norm();
        match (gyro_norm, accelerometer) {
            (Ok(gyro_norm), Ok(accelerometer)) => {
                println!("Gyro norm: {gyro_norm:?}; Accel: {accelerometer:?}")
            }
            (Err(gyro_err), Ok(accelerometer)) => {
                println!("Gyro err: {gyro_err:?}");
                println!("Accelerometer: {accelerometer:?}");
            }
            (Ok(gyro_norm), Err(accel_err)) => {
                println!("Gyro norm: {gyro_norm:?}");
                println!("Accelerometer error: {accel_err:?}");
            }
            (Err(gyro_err), Err(accel_err)) => {
                println!("Gyro error: {gyro_err:?}");
                println!("Accelerometer error: {accel_err:?}");
            }
        }
        Timer::after(Duration::from_millis(1000)).await;
        // }
    }
}

#[embassy_executor::task]
async fn run_usb_serial_jtag(mut usb_serial: UsbSerialJtag<'static>) {
    let mut label_1_value = 100;
    let mut label_2_value = 0.5;
    loop {
        let mut string = heapless::String::<512>::new();
        string
            .write_fmt(format_args!(
                "Label_1:{label_1_value},Label_2:{label_2_value}\n"
            ))
            .unwrap();

        match usb_serial.write_all(string.as_bytes()).await {
            Ok(()) => {
                println!("Wrote message: {string}");
            }
            Err(err) => println!("Error: {err}"),
        }
        // increment values
        label_1_value += 1;
        label_2_value += 0.1;
        Timer::after(Duration::from_secs(2)).await;
    }
}
#[embassy_executor::task]
async fn run_uart_plotter(mut uart: Uart<'static, UART1>) {
    let mut label_1_value = 100;
    let mut label_2_value = 0.5;
    loop {
        let mut string = heapless::String::<512>::new();
        string
            .write_fmt(format_args!(
                "Label_1:{label_1_value},Label_2:{label_2_value:.5}\n"
            ))
            .unwrap();

        match uart.write_all(string.as_bytes()).await {
            Ok(()) => {
                println!("Wrote message: {string}");
            }
            Err(err) => println!("Error: {err:?}"),
        }
        // increment values
        label_1_value += 1;
        label_2_value += 0.1;
        Timer::after(Duration::from_secs(2)).await;
    }
}

/// We are using the BMP388 barometric pressure sensor using a DFRobot
///
///
/// ## DFRobot BMP388 board
/// Product wiki page: https://wiki.dfrobot.com/Gravity_BMP280_Barometric_Pressure_Sensors_SKU_SEN0251
/// Schematics: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/%5BSEN0251%5D(V1.0)-SCH.pdf
/// DFRobot Datasheet of BMP388: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/BST-BMP388-DS001-01-1307765.pdf
#[embassy_executor::task]
async fn run_pressure_sense(i2c: &'static I2C0Mutex, mut delay: embassy_time::Delay) {
    println!("Here");
    let i2c_device = I2cDevice::new(i2c);
// async fn run_pressure_sense(i2c_device: I2C<'static, I2C0>, mut delay: embassy_time::Delay) {
    async fn log_sensor_settings(pressure_sensor: &mut BMP388<I2C0DeviceType, bmp388::Async>) {
        let sampling_rate = pressure_sensor.sampling_rate().await.unwrap();
        println!("Pressure sensor sampling rate: {sampling_rate:?}");
        let power_control = pressure_sensor.power_control().await.unwrap();
        println!("Pressure sensor power control: {power_control:?}");
        let status = pressure_sensor.status().await.unwrap();
        println!("Pressure sensor status: {status:?}");
        let oversampling = pressure_sensor.oversampling().await.unwrap();
        println!("Pressure sensor oversampling: {oversampling:?}");
        let filter = pressure_sensor.filter().await.unwrap();
        println!("Pressure sensor filter: {filter:?}");
        let interrupt_config = pressure_sensor.interrupt_config().await.unwrap();
        // println!("Pressure sensor Interrupt config - output: {}; active high? {}; latch: {}; data ready interrupt enable? {}", interrupt_config.output);
        println!("Pressure sensor Interrupt config: {interrupt_config:?}");
    }

    let address = 0x77;
    let mut pressure_sensor = bmp388::BMP388::new(i2c_device, address, &mut delay)
        .await
        .unwrap();

    // before setting up all values
    log_sensor_settings(&mut pressure_sensor).await;
    // recommended oversampling for temperature when using x16/x32 for pressure is x2!
    // Even though they recommend other lower oversampling values for Drones, if we have a powered rocket
    // we want maximum oversampling!
    // pressure_sensor
    //     .set_oversampling(bmp388::OversamplingConfig {
    //         osr_p: bmp388::Oversampling::x32,
    //         osr4_t: bmp388::Oversampling::x2,
    //     })
    //     .unwrap();
    // recommended PowerMode for drones is Normal

    // async fn force(sensor: &mut bmp388::BMP388<I2C<'static, I2C0>, bmp388::Async>) {
    async fn force(sensor: &mut bmp388::BMP388<I2C0DeviceType, bmp388::Async>) {
        sensor
            .set_power_control(bmp388::PowerControl {
                pressure_enable: true,
                temperature_enable: true,
                mode: bmp388::PowerMode::Normal,
            })
            .await
            .unwrap()
    }
    force(&mut pressure_sensor).await;
    // pressure_sensor.set_filter(bmp388::Filter::c127).unwrap();
    // pressure_sensor
    //     .set_interrupt_config(bmp388::InterruptConfig {
    //         output: bmp388::OutputMode::PushPull,
    //         active_high: true,
    //         latch: false,
    //         data_ready_interrupt_enable: true,
    //     })
    //     .unwrap();
    // After setting up all values
    log_sensor_settings(&mut pressure_sensor).await;

    println!("BMP388 pressure sensor initialised!");

    loop {
        // force(&mut pressure_sensor);
        let status = pressure_sensor.status().await.unwrap();
        let data = pressure_sensor.sensor_values().await.unwrap();
        // if status.pressure_data_ready && status.temperature_data_ready {

        println!(
            "(bmp388 sensor_values): Pressure: {}; Temperature: {} (Status: {:?})",
            data.pressure, data.temperature, status
        );
        // } else {

        //     println!("Pressure sensor status: command ready? {}; pressure data ready? {}; temperature data ready? {}", status.command_ready, status.pressure_data_ready, status.temperature_data_ready);
        // }
        // 10 ms = 100 Hz
        Timer::after(Duration::from_millis(10)).await;
    }
}
