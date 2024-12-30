use core::{fmt::Write as _, ops::Deref};

#[cfg(feature = "run-pressure-and-temperature")]
use bmp388::BMP388;
use esp_hal_embassy::Executor;
#[cfg(feature = "run-gnss")]
use lc76g::GnssMessage;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_futures::{join::join, select};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::Channel,
    mutex::Mutex,
    pipe::Pipe,
};
use embassy_time::{Duration, Timer};

use esp_hal::{
    clock::Clocks,
    delay::Delay,
    gpio::{GpioPin, Input, Io, Level, Output},
    i2c::{self, master::I2c},
    interrupt::{self, InterruptHandler, Priority},
    peripherals::{Interrupt, Peripherals, I2C0, UART0, UART1, USB_DEVICE},
    prelude::*,
    rng::Rng,
    rtc_cntl::Rtc,
    timer::timg::TimerGroup,
    uart::{self, Uart},
    usb_serial_jtag::UsbSerialJtag,
    Async,
};

use embedded_io_async::{Read, Write};

#[cfg(feature = "run-imu")]
use icm42670::accelerometer::Accelerometer;

use defmt::{debug, error, info, trace, warn};
#[cfg(feature = "run-gnss")]
use nmea::ParseResult;
use static_cell::StaticCell;

/// The Rust ESP32-C3 board has onboard LED on GPIO 7
pub type OnboardLed = Output<'static, GpioPin<7>>;

static MOCK_SENTENCES: &'static str = include_str!("../../../tests/nmea.log");

// Uart rx_fifo_full_threshold
const UART_READ_BUF_SIZE: usize = 126;

// EOT (CTRL-D)
const UART_AT_CMD: u8 = 0x04;

pub const NMEA_SENTENCE_TERMINATOR: &str = "\r\n";

pub type I2C0AsyncDeviceType =
    I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, esp_hal::Async, I2C0>>;
pub type I2C0AsyncMutex = Mutex<CriticalSectionRawMutex, I2c<'static, esp_hal::Async, I2C0>>;

pub type I2C0BlockingMutex =
    critical_section::Mutex<core::cell::RefCell<I2c<'static, esp_hal::Blocking, I2C0>>>;
pub type I2C0BlockingDeviceType =
    embedded_hal_bus::i2c::CriticalSectionDevice<'static, I2c<'static, esp_hal::Async, I2C0>>;

pub type I2C0Mutex = I2C0AsyncMutex;
pub type I2C0DeviceType = I2C0AsyncDeviceType;

/// GNSS: RST pin
// pub type GnssRSTPin = GpioPin<19><Output<PushPull>>;
pub type GnssRSTPin = GpioPin<19>;

/// GNSS: Board to GNSS TX pin is 5
// pub type GnssRXPin = Gpio5<Output<PushPull>>;
pub type GnssRXPin = GpioPin<5>;

/// GNSS: GNSS to board RX pin is 6
// pub type GnssTXPin = Gpio6<Input<Floating>>;
pub type GnssTXPin = GpioPin<6>;

#[cfg(feature = "run-gnss")]
pub type GnssUartSenderChannel = Channel<CriticalSectionRawMutex, GnssMessage, 100>;

/// Sd Card: Board to card Data In (DI) Pin
///
/// SPI MOSI pin
pub type SDCardDIPin = GpioPin<1>;

/// Sd Card: Card to board Data Out (DO) Pin
///
/// SPI MISO pin
// pub type SDCardDOPin = Gpio2<Input<PushPull>>;
pub type SDCardDOPin = GpioPin<2>;

/// Sd Card: SCLK (Clock) SPI Pin
///
/// SPI Clock Pin
// pub type SDCardCLKPin = Gpio3<Input<PushPull>>;
pub type SDCardCLKPin = GpioPin<3>;

/// Sd Card: Chip Select Pin for SD card
// pub type SDCardCSPin = Gpio1<Input<PushPull>>;
pub type SDCardCSPin = GpioPin<1>;

pub type DebugUartPipe = Pipe<NoopRawMutex, 1024>;

// pub type I2C0Mutex = I2C0BlockingMutex;
// pub type I2C0DeviceType = I2C0BlockingDeviceType;

pub struct Application {
    // TODO: Uncomment when you create a `Uart` instance of the `UART0` peripheral
    // uart: Uart<'static, UART0>,
    // TODO: Uncomment when you create a `Uart` instance of the `UART1` peripheral
    #[cfg(feature = "run-gnss")]
    gnss_uart: Uart<'static, UART1>,
    // TODO: Uncomment when you create a `Rng` instance
    rng: Rng,
    // TODO: Uncomment when you create the `OnboardLed` instance
    onboard_led: OnboardLed,
    // TODO: Uncomment when you create the `UsbSerialJtag` instance
    usb_serial_jtag: UsbSerialJtag<'static, Async>,
    // #[cfg(any(feature = "run"))]
    #[cfg(feature = "run-pressure-and-temperature")]
    i2c: &'static I2C0Mutex,
    #[cfg(feature = "run-temperature-and-humidity")]
    i2c: I2c<'static, esp_hal::Blocking, I2C0>,
    // i2c: &'static I2C0BlockingMutex,
}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init(peripherals: Peripherals) -> Self {
        let mut rtc = Rtc::new(peripherals.LPWR);
        let mut timer_group0 = TimerGroup::new(peripherals.TIMG0);
        let mut wdt0 = timer_group0.wdt;
        let timer_group1 = TimerGroup::new(peripherals.TIMG1);
        let mut wdt1 = timer_group1.wdt;

        // Disable watchdog timers
        rtc.swd.disable();
        rtc.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        esp_hal_embassy::init(timer_group0.timer0);

        // Setup peripherals for application
        // let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

        // Onboard LED
        // Rust ESP32-C3 schematics: https://raw.githubusercontent.com/esp-rs/esp-rust-board/master/assets/rust_board_v1_pin-layout.png
        // Set GPIO7 as an output, and set its state high initially.
        let mut onboard_led = Output::new_typed(peripherals.GPIO7, Level::High);

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
            uart::Config {
                // default baudrate
                // https://files.waveshare.com/upload/0/06/Quectel_LC26G%26LC76G%26LC86G_GNSS_Protocol_Specification_V1.0.0_Preliminary.pdf
                baudrate: 115200,
                data_bits: uart::DataBits::DataBits8,
                parity: uart::Parity::ParityNone,
                stop_bits: uart::StopBits::STOP1,
                rx_fifo_full_threshold: UART_READ_BUF_SIZE as u16,
                ..Default::default()
            },
            peripherals.GPIO6,
            peripherals.GPIO5,
        )
        .unwrap()
        .into_async();
        interrupt::enable(Interrupt::UART1, Priority::Priority1).unwrap();

        // let uart0 = {
        //     let mut uart0 = Uart::new_with_config(
        //         peripherals.UART0,
        //         uart::config::Config {
        //             baudrate: 115200,
        //             data_bits: uart::config::DataBits::DataBits8,
        //             parity: uart::config::Parity::ParityNone,
        //             stop_bits: uart::config::StopBits::STOP1,
        //         },
        //         Some(uart::TxRxPins::new_tx_rx(
        //             io.pins.gpio0.into_push_pull_output(),
        //             io.pins.gpio1.into_floating_input(),
        //         )),
        //         &clocks,
        //     );
        //     uart0
        //         .set_rx_fifo_full_threshold(UART_READ_BUF_SIZE as u16)
        //         .unwrap();
        //     interrupt::enable(Interrupt::UART0, Priority::Priority1).unwrap();

        //     uart0
        // };

        let mut usb_serial_jtag = UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();
        usb_serial_jtag.listen_rx_packet_recv_interrupt();
        interrupt::enable(Interrupt::USB_DEVICE, interrupt::Priority::Priority1).unwrap();

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

        #[cfg(feature = "run-pressure-and-temperature")]
        let i2c = {
            let i2c0 = I2c::new(
                peripherals.I2C0,
                i2c::master::Config {
                    frequency: 400_u32.kHz(),
                    ..Default::default()
                },
            )
            .with_sda(peripherals.GPIO10)
            .with_scl(peripherals.GPIO18)
            .into_async();
            interrupt::enable(Interrupt::I2C_EXT0, interrupt::Priority::Priority2).unwrap();

            static INSTANCE: StaticCell<Mutex<CriticalSectionRawMutex, I2c<'static, Async>>> =
                StaticCell::new();

            INSTANCE.init(Mutex::new(i2c0))
        };
        #[cfg(feature = "run-temperature-and-humidity")]
        let i2c = {
            let i2c0 = I2c::new_typed(
                peripherals.I2C0,
                i2c::master::Config {
                    frequency: 400_u32.kHz(),
                    ..Default::default()
                },
            )
            .with_sda(peripherals.GPIO10)
            .with_scl(peripherals.GPIO18);
            interrupt::enable(Interrupt::I2C_EXT0, interrupt::Priority::Priority2).unwrap();

            i2c0
        };
        info!("Peripherals initialized");
        Self {
            // uart: uart0,
            #[cfg(feature = "run-gnss")]
            gnss_uart: uart1,
            usb_serial_jtag,
            rng,
            onboard_led,
            #[cfg(any(feature = "run-temperature-and-humidity", feature = "run-pressure-and-temperature"))]
            i2c,
        }
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self) -> ! {
        let executor = {
            static INSTANCE: StaticCell<Executor> = StaticCell::new();

            INSTANCE.init(Executor::new())
        };

        executor.run(|spawner| {
            let status_channel = {
                static INSTANCE: StaticCell<StatusChannel> = StaticCell::new();
                INSTANCE.init(StatusChannel::new())
            };

            let uart_pipe = {
                static INSTANCE: StaticCell<DebugUartPipe> = StaticCell::new();
                INSTANCE.init(DebugUartPipe::new())
            };

            spawner.must_spawn(run_blinky(self.onboard_led, status_channel));
            spawner.must_spawn(run_uart_plotter(self.usb_serial_jtag, uart_pipe));
            // spawner.must_spawn(run_gnss_mocked(self.rng));

            #[cfg(feature = "run-gnss")]
            {
                let gnss_send_channel = {
                    static INSTANCE: StaticCell<GnssUartSenderChannel> = StaticCell::new();
                    INSTANCE.init(GnssUartSenderChannel::new())
                };

                let gnss_handler_channel = {
                    static INSTANCE: StaticCell<GnssHandlerChannel> = StaticCell::new();
                    INSTANCE.init(GnssHandlerChannel::new())
                };

                spawner.must_spawn(run_gnss(
                    self.gnss_uart,
                    gnss_send_channel,
                    gnss_handler_channel,
                ));
                spawner.must_spawn(run_gnss_setup(gnss_send_channel));
                spawner.must_spawn(run_gnss_handler(gnss_handler_channel));
            }

            #[cfg(feature = "run-imu")]
            {
                spawner.must_spawn(run_imu(self.i2c));
            }

            #[cfg(feature = "run-temperature-and-humidity")]
            spawner.must_spawn(run_temp_humid(
                // I2cDevice::new(self.i2c),
                self.i2c,
                esp_hal::delay::Delay::new(),
            ));

            // spawner.must_spawn(run_usb_serial_jtag(self.usb_serial_jtag));
            // spawner.must_spawn(run_usb_serial(self.usb_serial));
            #[cfg(feature = "run-pressure-and-temperature")]
            spawner.must_spawn(run_pressure_sense(self.i2c, embassy_time::Delay, uart_pipe));
        })
    }
}

// pub trait BlinkLed {
//     async fn blink_led();
// }

pub enum Status {}

// impl Status {
//     pub async fn blink_led(&self, led: OnboardLed) {
//         match self {

//         }
//     }
// }
pub enum Error {}

pub type StatusChannel = Channel<CriticalSectionRawMutex, Result<Status, Error>, 10>;

#[cfg(feature = "run-gnss")]
pub type GnssHandlerChannel =
    Channel<CriticalSectionRawMutex, heapless::Vec<nmea::ParseResult, 10>, 10>;

/// # Exercise: Flashing Onboard LED based on status
#[embassy_executor::task]
async fn run_blinky(mut led: OnboardLed, status_channel: &'static StatusChannel) {
    // LED Blinking Code goes here

    // Make an infinite loop
    loop {
        let status_res = status_channel.receive().await;

        match status_res {
            Ok(status) => {
                // Turn on the LED
                led.set_high();
                // Delay 200 ms
                Timer::after(Duration::from_millis(200)).await;
                // Turn off the LED
                led.set_low();
                // Delay 200 ms
                Timer::after(Duration::from_millis(200)).await;
            }
            Err(_err) => {
                // 1 seconds fast blinking
                for _ in 0..5 {
                    // Turn on the LED
                    led.set_high();
                    // Delay 200 ms
                    Timer::after(Duration::from_millis(100)).await;
                    // Turn off the LED
                    led.set_low();
                    // Delay 200 ms
                    Timer::after(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

#[cfg(feature = "run-gnss")]
mod gnss {
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
                    info!("GSA: Fix satellites: {:?}", gsa_data.fix_sats_prn);
                }
                Ok(ParseResult::GSV(gsv_data)) => {
                    info!("GSV: Satellites in view: {}", gsv_data.sats_in_view);
                }
                _ => {}
            }
            Timer::after(Duration::from_secs(2)).await;
        }
    }

    /// Sets some options for the GNSS receiver
    ///
    /// Sends the message over the channel [`GnssUartSenderChannel`] to the [`run_gnss`] task.
    #[embassy_executor::task]
    async fn run_gnss_setup(send_channel: &'static GnssUartSenderChannel) {
        let wait_for = Duration::from_millis(50);
        info!(
            "GNSS send channel message: Wait {} milliseconds before sending...",
            wait_for.as_millis()
        );
        Timer::after(wait_for).await;
        let baudrate = GnssMessage::SetBaudrate;
        info!("Sending: {}", baudrate.to_nmea_sentence());
        send_channel.send(baudrate).await;

        let gnss_providers = GnssMessage::EnableGnssProviders;
        info!("Sending: {}", gnss_providers.to_nmea_sentence());
        send_channel.send(gnss_providers).await;
    }

    fn split_sentences(sentences: &str) -> Option<Lines> {
        let (full_sentences, partial_sentence) = sentences.rsplit_once("\r\n").unwrap();

        let full_sentences = full_sentences
            .lines()
            .map(|line| {
                trace!("NMEA Sentence: {}", line);

                match nmea::parse_str(line) {
                    Ok(x) => Ok(x),
                    Err(err) => {
                        debug!(
                            "Failed to parse sentence because: {}; sentence: '{}'",
                            err, line
                        );
                        Err(err)
                    }
                }
            })
            .collect();

        if !partial_sentence.is_empty() {
            Some(Lines {
                partial_sentence: Some(partial_sentence),
                parsed: full_sentences,
            })
        } else {
            Some(Lines {
                partial_sentence: None,
                parsed: full_sentences,
            })
        }
    }

    #[derive(Debug)]
    #[cfg(feature = "run-gnss")]
    pub struct Lines<'a> {
        partial_sentence: Option<&'a str>,
        parsed: heapless::Vec<Result<nmea::ParseResult, nmea::Error<'a>>, 10>,
    }

    /// https://www.waveshare.com/wiki/LC76G_GNSS_Module
    #[embassy_executor::task]
    #[cfg(feature = "run-gnss")]
    async fn run_gnss(
        uart: Uart<'static, UART1>,
        send_channel: &'static GnssUartSenderChannel,
        gnss_handler_sender: &'static GnssHandlerChannel,
    ) {
        let (mut tx, mut rx) = uart.split();

        let receive = async {
            info!("GNSS Receive: Uart reading...");
            // max message size to receive
            // leave some extra space for AT-CMD characters
            const MAX_BUFFER_SIZE: usize = 3 * UART_READ_BUF_SIZE + 16;

            let mut rbuf: [u8; MAX_BUFFER_SIZE] = [0_u8; MAX_BUFFER_SIZE];
            let mut sentences_string = heapless::String::<512>::new();
            // let mut offset: usize = 0;
            loop {
                let r = embedded_io_async::Read::read(&mut rx, &mut rbuf).await;

                match r {
                    Ok(len) => {
                        match core::str::from_utf8(&rbuf[..len]) {
                            Ok(ascii_data) => {
                                defmt::info!("GNSS receive: Read {} bytes: {}", len, ascii_data);

                                // should fit the String buffer
                                sentences_string.push_str(ascii_data).unwrap();
                            }
                            Err(utf8_err) => {
                                error!(
                                    "GNSS receive: Failed to parse received GNSS bytes as utf8: {}",
                                    utf8_err
                                );
                                defmt::warn!(
                                "GNSS receive: We've cleared buffer, losing the following content from GNSS: '{}'",
                                sentences_string
                            );
                                sentences_string.clear();
                                continue;
                            }
                        };
                    }
                    Err(e) => {
                        defmt::error!("GNSS receive: RX Error: {:?}", e);
                        defmt::warn!(
                        "GNSS receive: We've cleared buffer, losing the following content from GNSS: '{}'",
                        sentences_string
                    );
                        sentences_string.clear();

                        continue;
                    }
                }

                if sentences_string.contains("\r\n") {
                    let (partial_sentence, sentences) =
                        match split_sentences(sentences_string.as_str()) {
                            Some(lines) => {
                                let partial_sentence = lines.partial_sentence.map(|string| {
                                    heapless::String::<250>::try_from(string).unwrap()
                                });
                                let sentences = lines
                                    .parsed
                                    .into_iter()
                                    .filter_map(|result| match result {
                                        Ok(sentence) => Some(sentence),
                                        Err(err) => {
                                            trace!("GNSS receive, sentence parsing: {}", &err);
                                            None
                                        }
                                    })
                                    .collect::<heapless::Vec<nmea::ParseResult, 10>>();

                                (partial_sentence, sentences)
                            }
                            None => {
                                continue;
                            }
                        };

                    if sentences.len() > 0 {
                        // info!(
                        //     "{} full NMEA sentences parsed: {:?}",
                        //     sentences.len(),
                        //     &sentences
                        // );
                        if let Err(_full_err) = gnss_handler_sender.try_send(sentences) {
                            warn!("GNSS sentences handler channel is full");
                        }
                    }
                    trace!(
                        "Partial NMEA sentence: {}",
                        partial_sentence.clone().unwrap_or_default().as_str()
                    );
                    sentences_string.clear();

                    if let Some(partial_sentence) = partial_sentence {
                        sentences_string
                            .push_str(partial_sentence.as_str())
                            .unwrap();
                    }
                }
            }
        };

        let send = async {
            info!("GNSS Send: Uart writing on a channel message");
            loop {
                let send_gnss_sentence = send_channel.receive().await;

                let sentence_string = send_gnss_sentence.to_nmea_sentence();
                info!(
                    "Sending sentence to Gnss receiver: '{}'",
                    sentence_string.trim_end()
                );
                match tx.write_all(sentence_string.as_bytes()).await {
                    Ok(_) => info!("GNSS sentence sent!"),
                    Err(err) => error!("GNSS UART send: {err:?}"),
                };
            }
        };

        select::select(receive, send).await;
    }

    #[embassy_executor::task]
    #[cfg(feature = "run-gnss")]
    async fn run_gnss_handler(gnss_handler_sender: &'static GnssHandlerChannel) {
        loop {
            let sentences = gnss_handler_sender.receive().await;
            for sentence in sentences {
                match sentence {
                    nmea::ParseResult::GSA(gsa) => {
                        info!(
                            "GSA - fixed sat prn ({} len): {:?}",
                            gsa.fix_sats_prn.len(),
                            gsa.fix_sats_prn
                        )
                    }
                    nmea::ParseResult::GSV(gsv) => {
                        info!(
                            "GSV - {}, sats in view: {}",
                            gsv.gnss_type, gsv.sats_in_view
                        )
                    }
                    nmea::ParseResult::RMC(rmc) => {
                        info!("RMC - status of fix: {:?}", rmc.status_of_fix)
                    }
                    nmea::ParseResult::GLL(gll) => {
                        info!(
                            "GLL - latitude: {:?}, longitude: {:?}, is valid? {}",
                            gll.latitude, gll.longitude, gll.valid
                        );
                    }
                    _ => {
                        // skip rest of the sentences
                    }
                }
            }
        }
    }

    #[cfg(feature = "run-gnss")]
    fn parse_sentence(sentence: &str) {
        // 1. Parse the sentences splitting them by `\r\n`

        // for sentence in sentences.split_terminator("\r\n") {

        use defmt::error;
        let parse_result = nmea::parse_str(sentence);

        // 2. Use a match on the result and handle the sentences:
        // - GSA
        // - GSV
        // - RMC
        match parse_result {
            Ok(ParseResult::GSA(gsa_data)) => {
                info!("{gsa_data:?}");
            }
            Ok(ParseResult::GSV(gsv_data)) => {
                info!("{gsv_data:?}");
            }
            Ok(ParseResult::RMC(rmc_data)) => {
                info!("{rmc_data:?}");
            }
            Err(err) => {
                error!("Error: {:?}; sentence: '{}'", err, sentence);
            }
            _ => {
                // skip
            }
        }
        // }
    }
}

/// # Exercise: Receive battery percentage over UART from the power-system
///
/// 1. create an infinite loop that
/// 2. tries to read the Battery Percentage value sent from the Power System,
/// 3. prints the value on success or the error on failure (using Debug formatting),
/// 4. Repeat the read every 20 milliseconds
#[embassy_executor::task]
async fn run_uart(uart: Uart<'static, Async, UART1>) {
    let (mut rx, mut tx) = uart.split();
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
        info!("Uart writing...");
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
        info!("Uart reading...");
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
                        Ok(received_str) => info!("Received String over UART: {}", received_str),
                        Err(err) => info!(
                            "UTF-8 error parsing UART bytes as string: {}",
                            defmt::Debug2Format(&err)
                        ),
                    }

                    offset = 0;
                }
                Err(e) => info!("RX Error: {:?}", e),
            }
        }
    };

    // receive.await
    join(receive, send).await;
}

/// 1 second
pub const MEASURE_TEMPERATURE_AND_HUMIDITY_EVERY: Duration = Duration::from_millis(500);

#[embassy_executor::task]
#[cfg(feature = "run-temperature-and-humidity")]
async fn run_temp_humid(
    i2c: I2c<'static, esp_hal::Blocking, I2C0>,
    // i2c: &'static I2C0BlockingMutex,
    // i2c: I2C0DeviceType,
    // i2c: &'static Mutex<CriticalSectionRawMutex, I2c<'static, I2C0>>,
    mut delay: esp_hal::delay::Delay,
) {
    // let i2c_device = I2C0DeviceType::new(i2c);
    // let i2c_device = I2cDevice::new(i2c);
    // let mut i2c = i2c.lock().await;

    // let mut sensor = shtcx::shtc3(i2c_device);
    let mut sensor = shtcx::shtc3(i2c);
    // let mut delay = esp_hal::Delay::new;

    let wait_for_measure_micros =
        shtcx::max_measurement_duration(&sensor, shtcx::PowerMode::NormalMode);
    let wait_for = Duration::from_micros(wait_for_measure_micros.into());

    let measure_every = MEASURE_TEMPERATURE_AND_HUMIDITY_EVERY - wait_for;
    loop {
        if let Err(err) = sensor.start_measurement(shtcx::PowerMode::NormalMode) {
            info!(
                "(shtc3::start_measurement) Error: {:?}",
                defmt::Debug2Format(&err)
            );
            continue;
        }

        Timer::after(wait_for).await;
        if let Err(err) = sensor.get_measurement_result() {
            info!(
                "(shtc3::start_measurement) Error: {:?}",
                defmt::Debug2Format(&err)
            );
            continue;
        }
        let combined = match sensor.measure(shtcx::PowerMode::NormalMode, &mut delay) {
            Ok(value) => {
                info!(
                    "Combined: {} °C / {} %RH",
                    value.temperature.as_degrees_celsius(),
                    value.humidity.as_percent()
                );
            }
            Err(err) => {
                info!(
                    "(shtc3::start_measure) Error: {:?}",
                    defmt::Debug2Format(&err)
                );
                // try again skipping the measure every time.
                continue;
            }
        };

        // println!("Temperature: {} °C", temperature.as_degrees_celsius());
        // println!("Humidity: {} %RH", humidity.as_percent());

        Timer::after(measure_every).await;
    }
}

/// 1 second
pub const MEASURE_IMU_EVERY: Duration = Duration::from_millis(1000);

/// IMU task for reading the Gyroscope and accelerometer data from the ICM-42670-P sensor.
///
/// ICM-42670-P Datasheet: <https://invensense.tdk.com/wp-content/uploads/2021/07/ds-000451_icm-42670-p-datasheet.pdf>
///
/// # Axes orientation
///
/// ![A screenshot of ICM-42670-P datasheet's 10.1 section for IMU axes orientation.](https://raw.githubusercontent.com/AeroRust/nanosat-workshop/2b4136ba7d6730f7dd342a5f4a9a9016f93137f8/docs/assets/onboard-computer-esp32c3-icm42670-p-orientation.png)
#[embassy_executor::task]
#[cfg(feature = "run-imu")]
async fn run_imu(i2c: &'static I2C0Mutex) {
    Timer::after(Duration::from_millis(1000)).await;

    info!("Initialize IMU Icm 42670...");
    loop {
        // Address::Primary is address `0x68`
        let i2c_device = I2cDevice::new(i2c);
        let mut imu = match icm42670::Icm42670::new(i2c_device, icm42670::Address::Secondary).await
        {
            Ok(imu) => imu,
            Err(err) => {
                error!("Error initializing IMU: {:?}", err);
                Timer::after(Duration::from_secs(5)).await;
                continue;
            }
        };

        loop {
            let gyro_norm = imu.gyro_norm_async().await;
            let accelerometer = imu.accel_norm_async().await;
            match (gyro_norm, accelerometer) {
                (Ok(gyro_norm), Ok(accelerometer)) => {
                    info!(
                        "IMU: Gyro norm: {:?}; Accel: {:?}",
                        gyro_norm, accelerometer
                    )
                }
                (Err(gyro_err), Ok(accelerometer)) => {
                    println!("IMU: Gyro err: {gyro_err:?}");
                    error!("IMU: Accelerometer: {:?}", accelerometer);
                }
                (Ok(gyro_norm), Err(accel_err)) => {
                    info!("IMU: Gyro norm: {:?}", gyro_norm);
                    error!("IMU: Accelerometer error: {:?}", accel_err);
                }
                (Err(gyro_err), Err(accel_err)) => {
                    error!("IMU: Gyro error: {:?}", gyro_err);
                    error!("IMU: Accelerometer error: {:?}", accel_err);
                }
            }
            Timer::after(MEASURE_IMU_EVERY).await;
        }
    }
}

#[embassy_executor::task]
async fn run_usb_serial_jtag(mut usb_serial: UsbSerialJtag<'static, Async>) {
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
                info!("Wrote message: {}", string);
            }
            Err(err) => info!("Error: {:?}", defmt::Debug2Format(&err)),
        }
        // increment values
        label_1_value += 1;
        label_2_value += 0.1;
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[embassy_executor::task]
// async fn run_uart_plotter(mut uart: Uart<'static, UART0>, uart_pipe: &'static DebugUartPipe) {
async fn run_uart_plotter(
    mut usb_serial_jtag: UsbSerialJtag<'static, Async>,
    uart_pipe: &'static DebugUartPipe,
) {
    loop {
        let mut buf = [0_u8; 512];
        let read = uart_pipe.read(&mut buf).await;

        match usb_serial_jtag.write_all(&buf[..read]).await {
            Ok(_) => {
                trace!("Wrote {} bytes to UART0 from Pipe", read);
            }
            Err(err) => warn!("Failed to write {} bytes to UART0: {:?}", read, err),
        }
    }

    // let mut label_1_value = 100;
    // let mut label_2_value = 0.5;
    // loop {
    //     let mut string = heapless::String::<512>::new();
    //     string
    //         .write_fmt(format_args!(
    //             "/*Label_1:{label_1_value},Label_2:{label_2_value:.5}*/\n"
    //         ))
    //         .unwrap();

    //     match uart.write_all(string.as_bytes()).await {
    //         Ok(()) => {
    //             println!("Wrote message: {string}");
    //         }
    //         Err(err) => println!("Error: {err:?}"),
    //     }
    //     // increment values
    //     label_1_value += 1;
    //     label_2_value += 0.1;
    //     Timer::after(Duration::from_secs(2)).await;
    // }
}

pub const MEASURE_PRESSURE_EVERY: Duration = Duration::from_millis(100);

/// Exercise: TBD
/// We are using the BMP388 barometric pressure sensor using a DFRobot breakout board
///
///
/// ## DFRobot BMP388 board
/// Product wiki page: https://wiki.dfrobot.com/Gravity_BMP280_Barometric_Pressure_Sensors_SKU_SEN0251
/// Schematics: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/%5BSEN0251%5D(V1.0)-SCH.pdf
/// DFRobot Datasheet of BMP388: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/BST-BMP388-DS001-01-1307765.pdf
#[embassy_executor::task]
#[cfg(feature = "run-pressure-and-temperature")]
async fn run_pressure_sense(
    i2c: &'static I2C0Mutex,
    mut delay: embassy_time::Delay,
    uart_pipe: &'static DebugUartPipe,
) {
    info!("(bmp388): Initialise BMP388 sensor...");
    // let i2c_device = I2cDevice::new(i2c);
    // async fn run_pressure_sense(i2c_device: I2c<'static, I2C0>, mut delay: embassy_time::Delay) {
    async fn log_sensor_settings(pressure_sensor: &mut BMP388<I2C0DeviceType, bmp388::Async>) {
        let sampling_rate = pressure_sensor.sampling_rate().await.unwrap();
        info!(
            "(bmp388): Pressure sensor sampling rate: {:?}",
            sampling_rate
        );
        let power_control = pressure_sensor.power_control().await.unwrap();
        info!(
            "(bmp388): Pressure sensor power control: {:?}",
            power_control
        );
        let status = pressure_sensor.status().await.unwrap();
        info!("(bmp388): Pressure sensor status: {:?}", status);
        let oversampling = pressure_sensor.oversampling().await.unwrap();
        info!("(bmp388): Pressure sensor oversampling: {:?}", oversampling);
        let filter = pressure_sensor.filter().await.unwrap();
        info!("(bmp388): Pressure sensor filter: {:?}", filter);
        let interrupt_config = pressure_sensor.interrupt_config().await.unwrap();
        info!(
            "(bmp388): Pressure sensor Interrupt config: {:?}",
            interrupt_config
        );
    }

    let address = 0x77;
    loop {
        let mut pressure_sensor =
            match bmp388::BMP388::new(I2C0DeviceType::new(i2c), address, &mut delay).await {
                Ok(sensor) => sensor,
                Err(err) => {
                    error!("(bmp388): Failed to initialise BMP388 sensor: {:?}", err);
                    Timer::after(Duration::from_secs(2)).await;
                    continue;
                }
            };

        info!("(bmp388): 2 Initialise BMP388 sensor...");

        // before setting up all values
        log_sensor_settings(&mut pressure_sensor).await;
        info!("(bmp388): 3 Initialise BMP388 sensor...");
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

        // async fn force(sensor: &mut bmp388::BMP388<I2c<'static, I2C0>, bmp388::Async>) {
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

        info!("(bmp388): BMP388 pressure sensor initialised!");

        let mut calibrated = false;

        loop {
            // force(&mut pressure_sensor);

            // let status = pressure_sensor.status().await.unwrap();
            let data = pressure_sensor.sensor_values().await;
            pub enum AltitudeMeasurement {
                Relative,
                SeaLevel,
            }

            let altitude = match pressure_sensor.altitude().await {
                Ok(x) => x,
                Err(err) => {
                    warn!(
                        "(bmp388 altitude): Failed to take altitude: '{:?}'. Try again...",
                        err
                    );
                    continue;
                }
            };
            let altitude = match (AltitudeMeasurement::SeaLevel, calibrated) {
                (AltitudeMeasurement::Relative, false) => {
                    info!("(bmp388): Calibrating at altitude {} meters", altitude);
                    let new_sea_level = match pressure_sensor
                        .calibrated_absolute_difference(altitude)
                        .await
                    {
                        Ok(x) => x,
                        Err(err) => {
                            warn!(
                                "(bmp388 altitude): Failed to calibrate '{:?}', try again...",
                                err
                            );
                            continue;
                        }
                    };
                    calibrated = true;
                    info!("(bmp388): New Sea level set at: {} Pa", new_sea_level);

                    altitude
                }
                (AltitudeMeasurement::Relative, true) | (AltitudeMeasurement::SeaLevel, _) => {
                    altitude
                }
            };

            match data {
                Ok(data) => {
                    if MEASURE_PRESSURE_EVERY > Duration::from_millis(500) {
                        info!(
                            "(bmp388 sensor_values): Pressure: {}; Temperature: {}; Altitude: {} m",
                            data.pressure, data.temperature, altitude
                        );
                    }
                    let mut uart_msg = heapless::String::<256>::new();
                    uart_msg
                        .write_fmt(format_args!(
                            "/*{},{},{}*/\n",
                            data.temperature, altitude, data.pressure
                        ))
                        .unwrap();

                    uart_pipe.write_all(uart_msg.as_bytes()).await
                }
                _ => {
                    // try again
                    continue;
                }
            }

            Timer::after(MEASURE_PRESSURE_EVERY).await;
        }
    }
}
