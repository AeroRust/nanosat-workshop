// #[cfg(feature = "rp2040")]
use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};

#[cfg(feature = "rp2040")]
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    i2c::{self, I2c, InterruptHandler},
    multicore::{spawn_core1, Stack},
    peripherals::{I2C0, PIN_25, PIN_4, UART0, UART1},
    uart::{
        self, BufferedInterruptHandler, BufferedUart, BufferedUartRx, Config, DataBits, Parity,
        StopBits,
    },
};

#[cfg(feature = "rp2040")]
bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
    I2C0_IRQ => InterruptHandler<I2C0>;
    // UART1_IRQ => BufferedInterruptHandler<UART1>;
});
use defmt::{error, info, unwrap, warn};

use static_cell::{make_static, StaticCell};

#[cfg(feature = "rp2040")]
/// Stack - Core1 stack = Core 0 stack size.
static CORE0_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(feature = "rp2040")]
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(feature = "rp2040")]
// TODO: Set a stack size for the second core
static mut CORE1_STACK: Stack<{ 30 * 1024 }> = Stack::new();

#[cfg(feature = "LC76G")]
pub mod gnss {
    // pub type UART_TX_PIN = 8
    // pub type UART_RX_PIN = 9
    // pub type UART_CSN_PIN =
}

#[cfg(feature = "BMP388")]
pub mod pressure_and_temperature {
    // pub type I2C_SDA_PIN= PIN_4
    // scl 5
}

pub struct Application {}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init(/* peripherals: Peripherals */) -> Self {
        // let system: SystemParts = peripherals.SYSTEM.split();
        // let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        // let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        // let mut timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
        // let mut wdt0 = timer_group0.wdt;
        // let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
        // let mut wdt1 = timer_group1.wdt;

        // // Disable watchdog timers
        // rtc.swd.disable();
        // rtc.rwdt.disable();
        // wdt0.disable();
        // wdt1.disable();

        // #[cfg(feature = "embassy-time-systick")]
        // embassy::init(
        //     &clocks,
        //     hal::systimer::SystemTimer::new(peripherals.SYSTIMER),
        // );

        // #[cfg(feature = "embassy-time-timg0")]
        // embassy::init(&clocks, timer_group0.timer0);

        // // Setup peripherals for application
        // let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // // Onboard LED
        // // Rust ESP32-C3 schematics: https://raw.githubusercontent.com/esp-rs/esp-rust-board/master/assets/rust_board_v1_pin-layout.png
        // // Set GPIO7 as an output, and set its state high initially.
        // let mut onboard_led = io.pins.gpio7.into_push_pull_output();
        // onboard_led.set_high().unwrap();

        // // Setup Random Generator for GNSS Reading
        // // Hal example: https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/rng.rs
        // let rng = Rng::new(peripherals.RNG);
        // // let rng = todo!("Initialize the Random generator");

        // // The Rust ESP32-C3 board has debug UART on pins 21 (TX) and 20 (RX)
        // // but you should use GPIO pins 0 (TX) and 1 (RX) for sending and receiving data respectively
        // // to/from the `power-system` board.
        // // TODO: Configure the UART 1 peripheral
        // // let mut uart1 = todo!("Configure UART 1 at pins 0 (TX) and 1 (RX) with `None` or default for the `Config`");

        // let mut uart1 = Uart::new_with_config(
        //     peripherals.UART1,
        //     uart::config::Config {
        //         // default baudrate
        //         // https://files.waveshare.com/upload/0/06/Quectel_LC26G%26LC76G%26LC86G_GNSS_Protocol_Specification_V1.0.0_Preliminary.pdf
        //         baudrate: 115200,
        //         data_bits: uart::config::DataBits::DataBits8,
        //         parity: uart::config::Parity::ParityNone,
        //         stop_bits: uart::config::StopBits::STOP1,
        //     },
        //     Some(uart::TxRxPins::new_tx_rx(
        //         io.pins.gpio5.into_push_pull_output(),
        //         io.pins.gpio6.into_floating_input(),
        //     )),
        //     &clocks,
        // );
        // uart1
        //     .set_rx_fifo_full_threshold(UART_READ_BUF_SIZE as u16)
        //     .unwrap();
        // interrupt::enable(Interrupt::UART1, Priority::Priority1).unwrap();

        // // let uart0 = {
        // //     let mut uart0 = Uart::new_with_config(
        // //         peripherals.UART0,
        // //         uart::config::Config {
        // //             baudrate: 115200,
        // //             data_bits: uart::config::DataBits::DataBits8,
        // //             parity: uart::config::Parity::ParityNone,
        // //             stop_bits: uart::config::StopBits::STOP1,
        // //         },
        // //         Some(uart::TxRxPins::new_tx_rx(
        // //             io.pins.gpio0.into_push_pull_output(),
        // //             io.pins.gpio1.into_floating_input(),
        // //         )),
        // //         &clocks,
        // //     );
        // //     uart0
        // //         .set_rx_fifo_full_threshold(UART_READ_BUF_SIZE as u16)
        // //         .unwrap();
        // //     interrupt::enable(Interrupt::UART0, Priority::Priority1).unwrap();

        // //     uart0
        // // };

        // let mut usb_serial_jtag = UsbSerialJtag::new(peripherals.USB_DEVICE);
        // usb_serial_jtag.listen_rx_packet_recv_interrupt();
        // // timer_group0.timer0.start(1u64.secs());
        // interrupt::enable(Interrupt::USB_DEVICE, interrupt::Priority::Priority1).unwrap();

        // // let usb = USB::new(
        // //     peripherals.USB0,
        // //     io.pins.gpio18,
        // //     io.pins.gpio19,
        // //     io.pins.gpio20,
        // // );

        // // let usb_bus = UsbBus::new(usb, unsafe { &mut EP_MEMORY });

        // // let mut serial = usbd_serial::SerialPort::new(&usb_bus);

        // // let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        // //     .manufacturer("esp-hal")
        // //     .product("esp-hal")
        // //     .serial_number("12345678")
        // //     .device_class(usbd_serial::USB_CLASS_CDC)
        // //     .build();

        // let i2c0 = I2C::new(
        //     peripherals.I2C0,
        //     io.pins.gpio10,
        //     io.pins.gpio8,
        //     400_u32.kHz(),
        //     &clocks,
        // );
        // interrupt::enable(Interrupt::I2C_EXT0, interrupt::Priority::Priority2).unwrap();

        // let i2c = make_static!(Mutex::<CriticalSectionRawMutex, _>::new(i2c0));

        // info!("Peripherals initialized");
        // Self {
        //     clocks,
        //     // uart: uart0,
        //     gnss_uart: uart1,
        //     usb_serial_jtag,
        //     rng,
        //     onboard_led,
        //     i2c,
        // }
        Self {}
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self/* , executor: &'static mut Executor */) -> ! {
        embassy_rp::pac::SIO.spinlock(31).write_value(1);

        let peripherals = embassy_rp::init(Default::default());
        let led = Output::new(peripherals.PIN_25, Level::Low);
        // connected to the UEXT pin for SPI 0 MISO
        // reset pin of the GNSS receiver
        let mut gnss_rst = Output::new(peripherals.PIN_4, Level::Low);

        let (tx_pin, rx_pin, uart) = (peripherals.PIN_0, peripherals.PIN_1, peripherals.UART0);

        // let (tx_pin, rx_pin, uart) = (peripherals.PIN_20, peripherals.PIN_21, peripherals.UART1);
        let tx_buf = &mut make_static!([0u8; 1024])[..];
        let rx_buf = &mut make_static!([0u8; 1024])[..];
        let uart = BufferedUart::new(
            uart,
            Irqs,
            tx_pin,
            rx_pin,
            tx_buf,
            rx_buf,
            // Config::default(),
            {
                let mut config = Config::default();
                config.baudrate = 115200;
                //  config.baudrate = 9600;
                config.data_bits = DataBits::DataBits8;
                config.stop_bits = StopBits::STOP1;
                config.parity = Parity::ParityNone;
                config
            },
        );

        // I2c for linking RP Pico PC with ESP32-C3 Rust board:
        // I2C0
        // SDA - GPIO 8
        // SCL - GPIO 9
        // let i2c0 = I2c::new_async(
        //     peripherals.I2C0,
        //     peripherals.PIN_9,
        //     peripherals.PIN_8,
        //     Irqs,
        //     {
        //         let mut config = i2c::Config::default();
        //         config.frequency = 400_000;
        //         config
        //     },
        // );
        spawn_core1(peripherals.CORE1, unsafe { &mut CORE1_STACK }, move || {
            let core1_executor = CORE1_EXECUTOR.init(Executor::new());

            core1_executor.run(|spawner| spawner.must_spawn(print()))
        });

        let core0_executor = CORE0_EXECUTOR.init(Executor::new());
        core0_executor.run(|spawner| {
            spawner.must_spawn(blinky(led));
            // spawner.must_spawn(read_uart(gnss_rst, uart));
            // spawner.must_spawn(run_imu(i2c0))
        })
    }
}

#[cfg(feature = "rp2040")]
#[embassy_executor::task()]
async fn print() {
    loop {
        info!("Printing on Core 1 every 2 secs...");
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[cfg(feature = "rp2040")]
#[embassy_executor::task()]
async fn blinky(mut led: Output<'static, PIN_25>) {
    loop {
        info!("led on!");
        led.set_high();
        Timer::after(Duration::from_secs(1)).await;

        info!("led off!");
        led.set_low();
        Timer::after(Duration::from_secs(1)).await;
    }
}

fn split_sentences(sentences: &str) -> Option<Lines> {
    let (full_sentences, partial_sentence) = sentences.rsplit_once("\r\n").unwrap();

    let full_sentences = full_sentences
        .lines()
        .map(|line| {
            info!("NMEA Sentence: {}", line);
            nmea::parse_str(line)
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
pub struct Lines<'a> {
    partial_sentence: Option<&'a str>,
    parsed: heapless::Vec<Result<nmea::ParseResult, nmea::Error<'a>>, 10>,
}

#[embassy_executor::task()]
async fn read_uart(mut gnss_rst: Output<'static, PIN_4>, uart: BufferedUart<'static, UART0>) {
    use embedded_io_async::BufRead;

    let (mut rx, mut tx) = uart.split();

    // Timer::after(Duration::from_secs(1)).await;
    // gnss_rst.set_low();
    // Timer::after(Duration::from_secs(1)).await;

    const BUF_SIZE: usize = 256;
    let mut buf_string = heapless::String::<{ BUF_SIZE }>::new();

    loop {
        let read_len = {
            info!("reading...");
            let read_buf = match rx.fill_buf().await {
                Ok(x) => x,
                Err(err) => {
                    error!("UART: {:?}", defmt::Debug2Format(&err));
                    warn!("Clear remaining buffer: '{}'", buf_string);
                    buf_string.clear();
                    continue;
                }
            };

            info!(
                "Read {} more bytes, string length so far: {}",
                read_buf.len(),
                buf_string.len()
            );
            let read_str = core::str::from_utf8(read_buf).unwrap();
            unwrap!(buf_string.push_str(read_str));
            info!("String messages:\n'{}'", buf_string);

            read_buf.len()
        };
        rx.consume(read_len);

        if buf_string.contains("\r\n") {
            let (partial_sentence, sentences) = match split_sentences(buf_string.as_str()) {
                Some(lines) => {
                    let partial_sentence = lines
                        .partial_sentence
                        .map(|string| unwrap!(heapless::String::<50>::try_from(string)));
                    let sentences = lines
                        .parsed
                        .into_iter()
                        .filter_map(|result| match result {
                            Ok(sentence) => Some(sentence),
                            Err(err) => {
                                error!("{}", defmt::Debug2Format(&err));
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

            info!(
                "Full sentences parsed: {:?}",
                defmt::Debug2Format(&sentences)
            );
            info!(
                "partial: {}",
                partial_sentence.clone().unwrap_or_default().as_str()
            );
            buf_string.clear();
            if let Some(partial_sentence) = partial_sentence {
                buf_string.push_str(partial_sentence.as_str()).unwrap();
            }
        }
    }
}
