use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::{
    asynch::i2c::I2cDevice, blocking::i2c::I2cDevice as BlockingI2cDevice,
};

// #[cfg(any(feature = "rp2040", feature = "rp23"))]
use embassy_executor::Executor;
use embassy_rp::{
    adc, bind_interrupts,
    gpio::{Level, Output, Pull},
    i2c::{self, I2c},
    multicore::{spawn_core1, Stack as MulticoreStack},
    peripherals::{
        ADC_TEMP_SENSOR, CORE1, DMA_CH0, I2C0, I2C1, PIN_23, PIN_24, PIN_29, PIN_5, PIN_7, PIO0,
        SPI0, UART0, UART1, USB,
    },
    pio::{self, Pio},
    spi::{self, Spi},
    uart::{
        self, BufferedInterruptHandler, BufferedUart, BufferedUartRx, Config, DataBits, Parity,
        StopBits,
    },
    usb::Driver,
};
#[cfg(any(feature = "rp2040", feature = "rp23"))]
use embassy_sync::{
    blocking_mutex::{
        raw::{CriticalSectionRawMutex, NoopRawMutex},
        Mutex as BlockingMutex,
    },
    channel::Channel,
    mutex::Mutex,
    pipe::Pipe,
};
use embassy_time::{Delay, Duration, Timer};

use static_cell::StaticCell;

use defmt::{error, info, trace, unwrap, warn};

#[cfg(any(feature = "rp2040", feature = "rp23"))]
bind_interrupts!(pub struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
    UART1_IRQ => BufferedInterruptHandler<UART1>;
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
    ADC_IRQ_FIFO => adc::InterruptHandler;
});

pub type I2C0DeviceType =
    I2cDevice<'static, CriticalSectionRawMutex, RefCell<I2c<'static, I2C0, i2c::Async>>>;
pub type I2C0Mutex =
    BlockingMutex<CriticalSectionRawMutex, RefCell<I2c<'static, I2C0, i2c::Async>>>;

pub type I2C1DeviceType =
    I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, I2C1, i2c::Async>>;
pub type I2C1Mutex = Mutex<CriticalSectionRawMutex, I2c<'static, I2C1, i2c::Async>>;

pub type SPI0Mutex = Mutex<CriticalSectionRawMutex, Spi<'static, SPI0, spi::Async>>;

#[cfg(any(feature = "rp2040", feature = "rp23"))]
/// Stack - Core1 stack = Core 0 stack size.
pub static CORE0_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(any(feature = "rp2040", feature = "rp23"))]
pub static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(any(feature = "rp2040", feature = "rp23"))]
// TODO: Set a stack size for the second core
pub static mut CORE1_STACK: MulticoreStack<{ 100 * 1024 }> = MulticoreStack::new();

pub struct Application {
    core1: CORE1,
    #[cfg(any(feature = "run-imu"))]
    i2c0: I2c<'static, I2C0, i2c::Async>,
    #[cfg(any(feature = "run-pressure-and-temperature"))]
    i2c1: I2c<'static, I2C1, i2c::Async>,
    #[cfg(feature = "run-gnss")]
    /// UART instance for GNSS receiver
    uart1: BufferedUart<'static, UART1>,
    #[cfg(feature = "run-gnss")]
    /// reset pin of the GNSS receiver
    gnss_rst: PIN_7,
    #[cfg(feature = "run-radio")]
    /// PWR pin, Pio SPI
    radio: (PIN_23, cyw43_pio::PioSpi<'static, PIO0, 0, DMA_CH0>),
    #[cfg(feature = "run-usb")]
    usb_driver: usb::MyUsbDriver,
    #[cfg(feature = "flash-store")]
    flash: flash::FlashType,
    #[cfg(feature = "run-storage")]
    spi0: Spi<'static, SPI0, spi::Async>,
    #[cfg(feature = "run-storage")]
    sdcard_csn: storage::SdCardCSnPin,
    // PIN_29 can be used for VSYS measurement but it's used by the WIFI too!
    #[cfg(feature = "run-status")]
    status: (ADC_TEMP_SENSOR, status::VSysSensePin, status::AdcType),
}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init() -> Self {
        embassy_rp::pac::SIO.spinlock(31).write_value(1);
        let peripherals = embassy_rp::init(Default::default());

        // add some delay to give an attached debug probe time to parse the
        // defmt RTT header. Reading that header might touch flash memory, which
        // interferes with flash write operations.
        // https://github.com/knurling-rs/defmt/pull/683
        #[cfg(not(feature = "run-imu"))]
        embassy_time::block_for(Duration::from_millis(10));

        // BNO055:
        // Initial startup delay
        // The sensor has an initial startup time during which interaction with it will fail.
        // As per the documentation this is in the 400ms - 650ms range (consult chapter 1.2 / page 14 for further details).
        // If your microcontroller is faster in starting up you might have to delay before talking to the sensor (or retry on failure).
        #[cfg(feature = "run-imu")]
        embassy_time::block_for(Duration::from_millis(650));

        #[cfg(feature = "run-imu")]
        let i2c0 = I2c::new_async(
            peripherals.I2C0,
            peripherals.PIN_17,
            peripherals.PIN_16,
            Irqs,
            {
                let mut config = i2c::Config::default();
                // 400 KHz
                config.frequency = 400_000;
                config
            },
        );

        #[cfg(feature = "run-pressure-and-temperature")]
        let i2c1 = I2c::new_async(
            peripherals.I2C1,
            peripherals.PIN_15,
            peripherals.PIN_14,
            Irqs,
            {
                let mut config = i2c::Config::default();
                // 400 KHz
                config.frequency = 400_000;
                config
            },
        );
        #[cfg(feature = "run-gnss")]
        let (gnss_rst, gnss_uart) = {
            // reset pin of the GNSS receiver
            let mut gnss_rst = peripherals.PIN_7;

            let (tx_pin, rx_pin, uart1) = (peripherals.PIN_8, peripherals.PIN_9, peripherals.UART1);

            let tx_buf = {
                static TX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
                TX_BUF.init([0u8; 1024])
            };

            let rx_buf = {
                static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
                RX_BUF.init([0u8; 1024])
            };
            let gnss_uart = BufferedUart::new(uart1, Irqs, tx_pin, rx_pin, tx_buf, rx_buf, {
                let mut config = Config::default();
                // default rate of the interface on LC76G
                config.baudrate = 115200;
                //  config.baudrate = 9600;
                config.data_bits = DataBits::DataBits8;
                config.stop_bits = StopBits::STOP1;
                config.parity = Parity::ParityNone;
                config
            });

            (gnss_rst, gnss_uart)
        };

        #[cfg(feature = "run-radio")]
        let cyw43_spi = {
            let mut pio = Pio::new(peripherals.PIO0, Irqs);
            let cs = Output::new(peripherals.PIN_25, Level::High);
            cyw43_pio::PioSpi::new(
                &mut pio.common,
                pio.sm0,
                pio.irq0,
                cs,
                peripherals.PIN_24,
                peripherals.PIN_29,
                peripherals.DMA_CH0,
            )
        };

        #[cfg(all(feature = "run-storage"))]
        // Obc
        let spi0 = Spi::new(
            peripherals.SPI0,
            peripherals.PIN_2,
            peripherals.PIN_3,
            peripherals.PIN_4,
            peripherals.DMA_CH4,
            peripherals.DMA_CH5,
            {
                let mut config = spi::Config::default();
                // start with 400 kHz to init the SD card
                // alter bump to 40 mHz
                config.frequency = 400_000;
                config
            },
        );

        #[cfg(feature = "run-usb")]
        // Create the driver, from the HAL.
        let usb_driver = Driver::new(peripherals.USB, Irqs);

        #[cfg(feature = "flash-store")]
        let flash = flash::FlashType::new(peripherals.FLASH, peripherals.DMA_CH1);

        #[cfg(feature = "run-status")]
        let adc = adc::Adc::new(peripherals.ADC, Irqs, adc::Config::default());

        info!("Peripherals initialised");

        Self {
            core1: peripherals.CORE1,
            #[cfg(any(feature = "run-imu"))]
            i2c0,
            #[cfg(any(feature = "run-pressure-and-temperature"))]
            i2c1,
            #[cfg(feature = "run-gnss")]
            gnss_rst,
            #[cfg(feature = "run-gnss")]
            uart1: gnss_uart,
            #[cfg(feature = "run-radio")]
            radio: (peripherals.PIN_23, cyw43_spi),
            #[cfg(feature = "run-usb")]
            usb_driver,
            #[cfg(feature = "flash-store")]
            flash,
            #[cfg(feature = "run-storage")]
            spi0,
            // OBC
            #[cfg(all(feature = "run-storage"))]
            sdcard_csn: peripherals.PIN_5,
            #[cfg(feature = "run-status")]
            status: (peripherals.ADC_TEMP_SENSOR, peripherals.PIN_28, adc),
        }
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self) -> ! {
        #[cfg(feature = "flash-store")]
        let flash_storage = {
            static FLASH_STORAGE: StaticCell<flash::FlashStorage> = StaticCell::new();
            info!("init: FlashStorage");
            &*FLASH_STORAGE.init(flash::FlashStorage::new(
                Mutex::<CriticalSectionRawMutex, _>::new(self.flash),
            ))
        };
        #[cfg(feature = "run-status")]
        let adc_mutex = {
            static ADC_MUTEX: StaticCell<status::AdcMutex> = StaticCell::new();
            info!("init: Adc");
            &*ADC_MUTEX.init(status::AdcMutex::new(self.status.2))
        };

        // Async APIs
        spawn_core1(
            self.core1,
            unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
            move || {
                let core1_executor = CORE1_EXECUTOR.init(Executor::new());

                core1_executor.run(|spawner| {
                    info!("init: core1 executor (non-blocking)");

                    #[cfg(any(feature = "run-pressure-and-temperature"))]
                    let i2c1_mutex = {
                        static I2C_MUTEX: StaticCell<I2C1Mutex> = StaticCell::new();
                        &*I2C_MUTEX.init(I2C1Mutex::new(self.i2c1))
                    };
                    #[cfg(any(feature = "run-storage"))]
                    let spi0_mutex = {
                        static SPI_MUTEX: StaticCell<SPI0Mutex> = StaticCell::new();
                        &*SPI_MUTEX.init(SPI0Mutex::new(self.spi0))
                    };

                    // spawner.must_spawn(blinky(led));
                    #[cfg(feature = "run-debug-uart")]
                    spawner.must_spawn(debug_uart::run_debug_uart());

                    #[cfg(feature = "run-radio")]
                    {
                        let (state, pwr, spi) = {
                            let pwr = self.radio.0;

                            static STATE: StaticCell<cyw43::State> = StaticCell::new();
                            let state = STATE.init(cyw43::State::new());

                            (state, pwr, self.radio.1)
                        };
                        // spawner.must_spawn(wifi::run_radio(  net_device, control, runner));
                        spawner.must_spawn(wifi::run_radio(
                            state,
                            pwr,
                            spi,
                            &wifi::RADIO_RECEIVE_CHANNEL,
                            &wifi::RADIO_SEND_CHANNEL,
                        ));
                    }

                    #[cfg(feature = "run-usb")]
                    {
                        // spawner.must_spawn(wifi::run_radio(  net_device, control, runner));
                        spawner.must_spawn(usb::run_usb(self.usb_driver));
                    }

                    #[cfg(feature = "run-pressure-and-temperature")]
                    {
                        spawner.must_spawn(bmp388::run_pressure_sense(i2c1_mutex, Delay));
                    }
                    #[cfg(feature = "run-gnss")]
                    {
                        static GNSS_SEND_CHANNEL: StaticCell<gnss::GnssUartSenderChannel> =
                            StaticCell::new();
                        let gnss_send_channel =
                            GNSS_SEND_CHANNEL.init(gnss::GnssUartSenderChannel::new());
                        static GNSS_HANDLER_CHANNEL: StaticCell<gnss::GnssHandlerChannel> =
                            StaticCell::new();
                        let gnss_handler_channel =
                            GNSS_HANDLER_CHANNEL.init(gnss::GnssHandlerChannel::new());

                        spawner.must_spawn(gnss::run_gnss(
                            self.gnss_rst,
                            self.uart1,
                            gnss_send_channel,
                            gnss_handler_channel,
                        ));
                        spawner.must_spawn(gnss::run_gnss_setup(gnss_send_channel));
                        spawner.must_spawn(gnss::run_gnss_handler(gnss_handler_channel));
                    }
                    #[cfg(feature = "run-status")]
                    {
                        spawner.must_spawn(status::run_status(
                            adc_mutex,
                            self.status.0,
                            self.status.1,
                        ));
                    }
                    #[cfg(feature = "run-storage")]
                    {
                        spawner.must_spawn(storage::run_storage(
                            &spi0_mutex,
                            self.sdcard_csn,
                            &storage::SD_CARD_CHANNEL,
                        ));
                    }
                })
            },
        );

        // Blocking APIs + Flash can be accessed only from core0 so we add the FlashStorage in Core0
        let core0_executor = CORE0_EXECUTOR.init(Executor::new());
        core0_executor.run(|spawner| {
            info!("init: core0 executor (blocking)");

            #[cfg(feature = "dummy-print")]
            spawner.must_spawn(print());

            #[cfg(any(feature = "run-imu"))]
            let i2c0_mutex = {
                static I2C0_MUTEX: StaticCell<I2C0Mutex> = StaticCell::new();
                &*I2C0_MUTEX.init(I2C0Mutex::new(RefCell::new(self.i2c0)))
            };

            #[cfg(any(feature = "run-imu"))]
            {
                spawner.must_spawn(bno055::run_imu(i2c0_mutex, flash_storage))
            }
        })
    }
}

#[cfg(any(feature = "rp2040", feature = "rp23"))]
#[embassy_executor::task()]
pub async fn print() {
    loop {
        info!("Printing on Core 1 every 2 secs...");
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[cfg(any(feature = "rp2040", feature = "rp23"))]
#[embassy_executor::task()]
pub async fn blinky(mut led: Output<'static>) {
    loop {
        info!("led on!");
        led.set_high();
        Timer::after(Duration::from_secs(1)).await;

        info!("led off!");
        led.set_low();
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[cfg(feature = "run-debug-uart")]
mod debug_uart {
    use embassy_sync::{blocking_mutex::raw::NoopRawMutex, pipe::Pipe};

    pub static DEBUG_UART_PIPE: DebugUartPipe = DebugUartPipe::new();

    pub type DebugUartPipe = Pipe<NoopRawMutex, 512>;

    #[embassy_executor::task]
    // pub async fn run_debug_uart(bytes_pipe: &'static DebugUartPipe) {
    pub async fn run_debug_uart() {
        loop {
            let mut buf = [0; 256];
            let bytes_read = DEBUG_UART_PIPE.read(&mut buf).await;
            let bytes_to_send = &buf[..bytes_read];

            // TODO: Send over Debug UART
        }
    }
}

#[cfg(feature = "flash-store")]
pub mod flash {

    #[cfg(feature = "BNO055")]
    use bno055::{BNO055Calibration, BNO055CalibrationStatus, BNO055_CALIB_SIZE};
    use defmt::*;

    use embassy_rp::{
        flash::{self, Async, Error, Flash},
        peripherals::FLASH,
    };
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};

    pub use embassy_rp::flash::ERASE_SIZE;

    /// Pico-w has 2 MB of flash
    pub const FLASH_SIZE: usize = 2 * 1024 * 1024;

    pub type FlashType = Flash<'static, FLASH, Async, FLASH_SIZE>;

    pub type FlashMutex = Mutex<CriticalSectionRawMutex, FlashType>;

    pub struct FlashStorage {
        flash: FlashMutex,
    }

    impl FlashStorage {
        #[cfg(feature = "BNO055")]
        pub const CALIBRATION_STATUS_SIZE: usize = core::mem::size_of::<BNO055CalibrationStatus>();
        #[cfg(feature = "BNO055")]
        pub const CALIBRATION_SIZE: usize = core::mem::size_of::<BNO055Calibration>();
        pub const FLASH_BLOCK_ADDR: u32 = 0x100000;
        #[cfg(feature = "BNO055")]
        pub const IMU_CALIBRATION_INDEX_OFFSET: usize = 0;

        pub fn new(flash: FlashMutex) -> Self {
            Self { flash }
        }

        #[cfg(feature = "BNO055")]
        pub async fn imu_calibration(&self) -> Result<Option<BNO055Calibration>, Error> {
            let mut guard = self.flash.lock().await;

            let mut buf = [0; ERASE_SIZE];
            let read_result = guard.blocking_read(Self::FLASH_BLOCK_ADDR, &mut buf);

            match read_result {
                Ok(_) => {
                    let status = Status(BNO055CalibrationStatus {
                        sys: buf[0],
                        gyr: buf[1],
                        acc: buf[2],
                        mag: buf[3],
                    });

                    info!("{:?}", status);

                    let mut profile_buf = [0; 22];
                    profile_buf.copy_from_slice(&buf[4..4 + Self::CALIBRATION_SIZE]);

                    let calibration = BNO055Calibration::from_buf(&profile_buf);
                    info!("{:?}", calibration);

                    let calibration = status
                        .is_calibrated()
                        .then(|| BNO055Calibration::from_buf(&profile_buf));

                    Ok(calibration)
                }
                Err(err) => {
                    error!(
                        "Failed to read Flash memory for IMU Calibration data: {}!",
                        err
                    );

                    Err(err)
                }
            }
        }

        #[cfg(feature = "BNO055")]
        /// Write the IMU calibration to flash, only if it has been fully calibrated
        ///
        /// # Returns
        /// false - if the passed Calibration status is not fully calibrated
        pub async fn write_imu_calibration(
            &self,
            status: BNO055CalibrationStatus,
            calibration: BNO055Calibration,
        ) -> Result<bool, Error> {
            let inner_status = Status(status);
            let mut guard = self.flash.lock().await;

            let mut buf = [0; ERASE_SIZE];
            // read the flash block first so we don't override any data at the rest of the block
            guard.blocking_read(Self::FLASH_BLOCK_ADDR, &mut buf)?;

            if inner_status.is_calibrated() {
                let calibration_buf = &mut buf[Self::IMU_CALIBRATION_INDEX_OFFSET..];

                {
                    // encode status, bytes 0..3
                    calibration_buf[..Self::CALIBRATION_STATUS_SIZE]
                        .copy_from_slice(&inner_status.to_bytes());

                    // encode actual calibration, bytes 4..26
                    calibration_buf[Self::CALIBRATION_STATUS_SIZE
                        ..Self::CALIBRATION_STATUS_SIZE + Self::CALIBRATION_SIZE]
                        .copy_from_slice(calibration.as_bytes());
                }

                if let Err(err) = guard.blocking_write(Self::FLASH_BLOCK_ADDR, &buf) {
                    error!(
                        "Failed to write new IMU calibration data to Flash memory: {}!",
                        err
                    );

                    Err(err)
                } else {
                    Ok(true)
                }
            } else {
                Ok(false)
            }
        }
    }

    #[cfg(feature = "BNO055")]
    #[derive(defmt::Format)]
    pub struct Status(bno055::BNO055CalibrationStatus);

    #[cfg(feature = "BNO055")]
    impl Status {
        pub fn is_calibrated(&self) -> bool {
            self.0.sys == 3 && self.0.gyr == 3 && self.0.acc == 3 && self.0.mag == 3
        }
        pub fn calibrated() -> Self {
            Self::from([3, 3, 3, 3])
        }

        pub fn to_bytes(self) -> [u8; 4] {
            self.into()
        }
    }
    #[cfg(feature = "BNO055")]
    impl From<[u8; 4]> for Status {
        fn from(array: [u8; 4]) -> Self {
            Self(BNO055CalibrationStatus {
                sys: array[0],
                gyr: array[1],
                acc: array[2],
                mag: array[3],
            })
        }
    }

    #[cfg(feature = "BNO055")]
    impl From<Status> for [u8; 4] {
        fn from(status: Status) -> Self {
            [status.0.sys, status.0.gyr, status.0.acc, status.0.mag]
        }
    }
}
#[cfg(feature = "cyw43")]
mod wifi {
    use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    use portable_atomic::AtomicBool;

    use embassy_net::{Config, Stack, StackResources};
    use embassy_rp::{
        gpio::Output,
        peripherals::{DMA_CH0, PIN_23, PIN_25, PIN_29, PIO0},
    };
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

    use {cyw43::JoinOptions, cyw43_pio::PioSpi};

    use protocol::{ReceiveMessage, SendPacket};

    /// Power pin for Cyw43
    pub type PinPWR = PIN_23;
    /// Chip select pin for the Spi of Cyw43
    pub type PinCs = PIN_25;

    pub type Cyw43Spi<'a> = PioSpi<'a, PIO0, 0, DMA_CH0>;
    pub type RadioSendChannel =
        embassy_sync::channel::Channel<CriticalSectionRawMutex, SendPacket, 100>;
    pub type RadioReceiveChannel =
        embassy_sync::channel::Channel<CriticalSectionRawMutex, ReceiveMessage, 100>;

    pub static RADIO_RECEIVE_CHANNEL: RadioReceiveChannel = RadioReceiveChannel::new();
    pub static RADIO_SEND_CHANNEL: RadioSendChannel = RadioSendChannel::new();

    #[cfg(not(feature = "demo-ground"))]
    pub const RECEIVER_IP: once_cell::sync::Lazy<SocketAddrV4> = once_cell::sync::Lazy::new(|| {
        SocketAddrV4::new(
            env!("WIFI_REMOTE_DESTINATION_IP")
                .parse::<Ipv4Addr>()
                .expect("Destination IP should be a valid Ipv4Addr!"),
            env!("WIFI_REMOTE_DESTINATION_PORT")
                .parse::<u16>()
                .expect("Port should be valid u16!"),
        )
    });
    #[cfg(feature = "demo-ground")]
    pub const RECEIVER_IP: once_cell::sync::Lazy<SocketAddrV4> = once_cell::sync::Lazy::new(|| {
        SocketAddrV4::new(
            env!("WIFI_REMOTE_DESTINATION_IP_2")
                .parse::<Ipv4Addr>()
                .expect("Destination IP should be a valid Ipv4Addr!"),
            env!("WIFI_REMOTE_DESTINATION_PORT_2")
                .parse::<u16>()
                .expect("Port should be valid u16!"),
        )
    });

    /// The WiFI network SSID to use for the WiFi `run-radio` feature
    /// Set in the `build.rs`` using a `.env` file in `onboard-computer`
    #[cfg(feature = "run-radio")]
    const WIFI_SSID: &str = env!("WIFI_SSID");
    /// The WiFI network password to use for the WiFi `run-radio` feature
    /// Set in the `build.rs`` using a `.env` file in `onboard-computer`
    #[cfg(feature = "run-radio")]
    const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

    /// An atomic signalling whether or not our device
    /// is connected over USB.
    pub static WIFI_CONNECTED: AtomicBool = AtomicBool::new(false);

    #[cfg(feature = "run-radio")]
    #[embassy_executor::task]
    pub async fn run_radio(
        state: &'static mut cyw43::State,
        pwr: PinPWR,
        spi: Cyw43Spi<'static>,
        receive_channel: &'static RadioReceiveChannel,
        send_channel: &'static RadioSendChannel,
    ) {
        use defmt::{error, info, unwrap, warn};
        use embassy_net::{
            udp::{PacketMetadata, UdpSocket},
            DhcpConfig, IpEndpoint, Ipv4Address,
        };
        use embassy_rp::{
            clocks::RoscRng,
            gpio::{Level, Output},
        };
        use embassy_time::{Duration, Timer};
        use static_cell::StaticCell;

        let mut rng = RoscRng;

        let pwr = Output::new(pwr, Level::Low);

        #[cfg(not(feature = "wifi-ap"))]
        info!(
            "(wifi): Setting up Radio - WiFi SSID: {} Password: {}",
            WIFI_SSID, WIFI_PASSWORD
        );

        #[cfg(feature = "flash-wifi-firmware")]
        let (fw, clm) = {
            let fw = include_bytes!("../../../cyw43-firmware/43439A0.bin");
            let clm = include_bytes!("../../../cyw43-firmware/43439A0_clm.bin");
            (fw, clm)
        };
        #[cfg(not(feature = "flash-wifi-firmware"))]
        let (fw, clm) = {
            // To make flashing faster for development, you may want to flash the firmwares independently
            // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
            //     probe-rs download 43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
            //     probe-rs download 43439A0_clm.bin --format bin --chip RP2040 --base-address 0x10140000
            let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
            let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };
            (fw, clm)
        };

        let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

        let spawner = embassy_executor::Spawner::for_current_executor().await;

        spawner.must_spawn(wifi_task(runner));
        info!("(wifi): WiFi task spawned");

        let (stack, runner) = {
            #[cfg(not(feature = "wifi-ap"))]
            let config = {
                let mut dhcp = DhcpConfig::default();
                dhcp.hostname = Some("nanosat-obc".parse::<heapless::String<32>>().unwrap());
                Config::dhcpv4(dhcp)
            };

            #[cfg(feature = "wifi-ap")]
            // Use a link-local address for communication without DHCP server
            let config = Config::ipv4_static(embassy_net::StaticConfigV4 {
                address: embassy_net::Ipv4Cidr::new(
                    embassy_net::Ipv4Address::new(192, 168, 1, 1),
                    // with default subnet mask of 255.255.255.0
                    // usable host IP range: 192.168.0.1 - 192.168.0.254
                    24,
                ),
                dns_servers: heapless::Vec::new(),
                gateway: None,
            });

            control.init(clm).await;
            control
                .set_power_management(cyw43::PowerManagementMode::Performance)
                .await;

            // Generate random seed
            let seed = rng.next_u64(); // chosen by fair dice roll. guaranteed to be random.

            // Init network stack
            // we use only 1 socket for now
            static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
            embassy_net::new(
                net_device,
                config,
                RESOURCES.init(StackResources::new()),
                seed,
            )
        };

        spawner.must_spawn(net_task(runner));
        info!("(wifi): Net task spawned");

        // set e.g. access point
        #[cfg(feature = "wifi-ap")]
        control
            .start_ap_wpa2("nanosat-obc", "OpenSourceFTW", 5)
            .await;

        // And now we can use it!
        let rx_buffer = {
            static RX_BUFFER: StaticCell<[u8; 2048]> = StaticCell::new();
            RX_BUFFER.init([0; 2048])
        };
        let tx_buffer = {
            static TX_BUFFER: StaticCell<[u8; 2048]> = StaticCell::new();
            TX_BUFFER.init([0; 2048])
        };
        let mut rx_meta = {
            static RX_META: StaticCell<[PacketMetadata; 16]> = StaticCell::new();
            RX_META.init([PacketMetadata::EMPTY; 16])
        };
        let mut tx_meta = {
            static TX_META: StaticCell<[PacketMetadata; 16]> = StaticCell::new();
            TX_META.init([PacketMetadata::EMPTY; 16])
        };

        let mut socket = UdpSocket::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        let bind_port_result = socket.bind(1234);
        if let Err(err) = bind_port_result {
            error!("(wifi): UDP Bind to port 1234 errored: {}", err);
        }

        unwrap!(bind_port_result);

        let socket = {
            static SOCKET: StaticCell<UdpSocket<'static>> = StaticCell::new();
            SOCKET.init(socket)
        };

        loop {
            #[cfg(not(feature = "wifi-ap"))]
            // when using AP we don't join a network, we create one
            // for now, we use static IP config and not DHCP like this part
            {
                loop {
                    match control
                        .join(WIFI_SSID, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
                        .await
                    {
                        Ok(_) => {
                            info!("(wifi): Joined WiFi with SSID: {}", WIFI_SSID);

                            // set status LED to high for "connected to WiFi" status
                            control.gpio_set(0, true).await;
                            WIFI_CONNECTED.store(true, portable_atomic::Ordering::SeqCst);
                            break;
                        }
                        Err(err) => {
                            warn!("(wifi): join failed with status={}", err.status);
                        }
                    }
                }

                // Wait for DHCP, not necessary when using static IP
                info!("(wifi): waiting for DHCP...");
                while !stack.is_config_up() {
                    Timer::after_millis(100).await;
                }
                info!("(wifi): DHCP is now up!");
                // TODO: set in Device status the Network connection config
                // stack.config_v4()
            }
            info!("{:#?}", defmt::Debug2Format(&stack.config_v4()));

            #[cfg(feature = "wifi-ap")]
            {
                info!("(wifi): set wifi as connected (AP mode)");

                // set status LED to high for "connected to WiFi" status
                control.gpio_set(0, true).await;
                WIFI_CONNECTED.store(true, portable_atomic::Ordering::SeqCst);
            }

            let send = async {
                let mut buf = [0; 2048];
                loop {
                    let send_packet = send_channel.receive().await;

                    let slice = match send_packet.message.to_radio(&mut buf) {
                        Ok(x) => x,
                        Err(err) => {
                            error!(
                                "Failed to serialise radio packet ({}): {}",
                                err, send_packet
                            );
                            continue;
                        }
                    };

                    let ip_endpoint = IpEndpoint::from((
                        Ipv4Address::from(send_packet.remote.0),
                        send_packet.remote.1,
                    ));
                    match socket.send_to(slice, ip_endpoint).await {
                        Ok(()) => {
                            defmt::debug!("packet sent to {}", defmt::Debug2Format(&ip_endpoint));
                        }
                        Err(e) => {
                            warn!("write error: {:?}", e);
                            continue;
                        }
                    };
                }
            };

            let receive = async {
                let mut buf = [0; 2048];

                loop {
                    let (n, _remote) = match socket.recv_from(&mut buf).await {
                        Ok(x) => x,
                        Err(e) => {
                            warn!("read error: {:?}", e);
                            continue;
                        }
                    };

                    let received_message: ReceiveMessage =
                        match ReceiveMessage::from_radio(&buf[..n]) {
                            Ok(message) => message,
                            Err(err) => {
                                error!(
                                    "Failed to deserialize received message: {}",
                                    defmt::Debug2Format(&err)
                                );
                                continue;
                            }
                        };

                    // do not block if channel is full
                    if let Err(err) = receive_channel.try_send(received_message) {
                        error!("Received radio packet is lost because Radio Receive Channel is full. Lost Message: {:?}", err)
                    }
                }
            };

            embassy_futures::select::select(send, receive).await;

            #[cfg(feature = "wifi-ap")]
            {
                info!("(wifi): set wifi as connected (AP mode)");

                // set status LED to high for "connected to WiFi" status
                control.gpio_set(0, false).await;
                WIFI_CONNECTED.store(false, portable_atomic::Ordering::SeqCst);
            }
        }
    }

    #[embassy_executor::task]
    #[cfg(feature = "run-radio")]
    async fn wifi_task(
        runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
    ) -> ! {
        runner.run().await
    }

    #[embassy_executor::task]
    #[cfg(feature = "run-radio")]
    async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
        runner.run().await
    }
}

#[cfg(feature = "usb")]
mod usb {
    use defmt::*;

    use embassy_rp::{peripherals::USB, usb::Driver};
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe};
    use embassy_usb::{class::cdc_acm::CdcAcmClass, driver::EndpointError, UsbDevice};

    use portable_atomic::{AtomicBool, Ordering};

    pub type UsbPipe = Pipe<CriticalSectionRawMutex, 256>;

    pub static USB_PIPE: UsbPipe = UsbPipe::new();

    pub type MyUsbDriver = Driver<'static, USB>;
    pub type MyUsbDevice = UsbDevice<'static, MyUsbDriver>;

    /// An atomic signalling whether or not our device
    /// is connected over USB.
    pub static USB_CONNECTED: AtomicBool = AtomicBool::new(false);

    #[embassy_executor::task]
    #[cfg(feature = "run-usb")]
    pub async fn run_usb(driver: MyUsbDriver) -> ! {
        use embassy_time::{with_timeout, Duration};
        use static_cell::StaticCell;

        pub const MAX_PACKET_SIZE: usize = 64;
        pub const MAX_PACKET_SIZE_U16: u16 = 64;
        pub const MAX_PACKET_SIZE_U8: u8 = 64;

        let spawner = embassy_executor::Spawner::for_current_executor().await;

        // Create embassy-usb Config
        let config = {
            // TODO: Change
            let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
            config.manufacturer = Some("AeroRust");
            config.product = Some("Nanosat OBC");
            // the year AeroRust was created
            config.serial_number = Some("20202020");
            // From 100 to 500 mA
            config.max_power = 100;
            // this is the maximum allowed packet size
            config.max_packet_size_0 = MAX_PACKET_SIZE_U8;

            // Required for windows compatibility.
            // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
            config.device_class = 0xEF;
            config.device_sub_class = 0x02;
            config.device_protocol = 0x01;
            config.composite_with_iads = true;
            config
        };

        // Create embassy-usb DeviceBuilder using the driver and config.
        // It needs some buffers for building the descriptors.
        let mut builder = {
            static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

            let builder = embassy_usb::Builder::new(
                driver,
                config,
                CONFIG_DESCRIPTOR.init([0; 256]),
                BOS_DESCRIPTOR.init([0; 256]),
                &mut [], // no msos descriptors
                CONTROL_BUF.init([0; 64]),
            );
            builder
        };

        // Create classes on the builder.
        let mut class = {
            static STATE: StaticCell<embassy_usb::class::cdc_acm::State> = StaticCell::new();
            let state = STATE.init(embassy_usb::class::cdc_acm::State::new());
            CdcAcmClass::new(&mut builder, state, MAX_PACKET_SIZE_U16)
        };

        // Build the builder.
        let usb = builder.build();

        // Run the USB device.
        spawner.must_spawn(usb_task(usb));

        loop {
            class.wait_connection().await;
            {
                USB_CONNECTED.store(true, Ordering::SeqCst);
            }
            info!("USB: Connected");

            'pipe: loop {
                let mut buf = [0; 256];

                let bytes_to_send = {
                    let bytes_read = USB_PIPE.read(&mut buf).await;
                    trace!("USB: Pipe bytes read: {}", bytes_read);
                    &buf[..bytes_read]
                };

                trace!("USB: to send {} bytes", bytes_to_send.len());

                let total_packets = bytes_to_send.len().div_ceil(MAX_PACKET_SIZE);
                for i in 0..total_packets {
                    let start_i = i * 64;
                    // we don't want to get out-of-bound for any leftover if
                    // we cannot divide the bytes to full packets
                    let end_i = (start_i + MAX_PACKET_SIZE).min(bytes_to_send.len());
                    let current_packet = &bytes_to_send[start_i..end_i];
                    trace!("Sending packet {}/{}", i + 1, total_packets);

                    trace!("Current packet length: {} bytes", current_packet.len());

                    // write this packet
                    // let res = with_timeout(Duration::from_millis(50), tx.send(&data)).await;
                    // match res {
                    //     Ok(Ok(_)) => /* all good */,
                    //     Ok(Err(_)) => /* sending error, not timeout */,
                    //     Err(_) => /* timeout */,
                    // }
                    // 1 ms = 1000000 ns
                    match with_timeout(
                        Duration::from_nanos(100_000),
                        class.write_packet(&bytes_to_send[start_i..end_i]),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            info!(
                                "Packet {}/{} sent ({} bytes)",
                                i + 1,
                                total_packets,
                                current_packet.len()
                            );
                        }
                        Ok(Err(EndpointError::BufferOverflow)) => {
                            error!("Buffer overflow!");
                        }
                        Ok(Err(EndpointError::Disabled)) => {
                            info!("USB: Disconnected");
                            break 'pipe;
                        }
                        Err(_) => {
                            warn!("USB: Packet sending Timeout")
                        }
                    }

                    // send zero-length packet (ZLP) when the **last** chunked packet length is exactly MAX_PACKET_SIZE, i.e. 64
                    if i == total_packets - 1
                        && bytes_to_send[start_i..end_i].len() % MAX_PACKET_SIZE == 0
                    {
                        debug!("Sending ZLP packet for {}/{}...", i + 1, total_packets);
                        match with_timeout(Duration::from_nanos(100_000), class.write_packet(&[]))
                            .await
                        {
                            Ok(Ok(_)) => {
                                info!("ZLP: Sent successfully")
                            }
                            Ok(Err(EndpointError::BufferOverflow)) => {
                                error!("ZLP: Buffer overflow!");
                            }
                            Ok(Err(EndpointError::Disabled)) => {
                                info!("USB: ZLP Disconnected");
                                break 'pipe;
                            }
                            Err(_) => {
                                warn!("USB: Packet sending Timeout")
                            }
                        }
                    }
                }
            }

            {
                USB_CONNECTED.store(false, Ordering::SeqCst);
            }
            info!("USB: Disconnected");
        }
    }

    #[embassy_executor::task]
    #[cfg(feature = "run-usb")]
    async fn usb_task(mut usb: MyUsbDevice) -> ! {
        usb.run().await
    }

    pub struct Disconnected {}

    impl From<EndpointError> for Disconnected {
        fn from(val: EndpointError) -> Self {
            match val {
                // TODO: handle the Buffer overflow instead of panicking!
                EndpointError::BufferOverflow => defmt::panic!("Buffer overflow"),
                EndpointError::Disabled => Disconnected {},
            }
        }
    }
}

#[cfg(feature = "BMP388")]
mod bmp388 {
    use core::fmt::Write as _;

    use defmt::*;

    use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
    use embassy_rp::{
        i2c::{self, I2c},
        peripherals::I2C0,
    };
    use embassy_sync::{
        blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
        mutex::Mutex,
    };
    use embassy_time::{Duration, Timer};

    use bmp388::BMP388;

    use super::{I2C0DeviceType, I2C0Mutex, I2C1DeviceType, I2C1Mutex};

    const MEASURE_PRESSURE_EVERY: Duration = Duration::from_millis(500);

    /// Exercise: TBD
    /// We are using the BMP388 barometric pressure sensor using a DFRobot breakout board
    ///
    ///
    /// ## DFRobot BMP388 board
    /// Product wiki page: https://wiki.dfrobot.com/Gravity_BMP280_Barometric_Pressure_Sensors_SKU_SEN0251
    /// Schematics: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/%5BSEN0251%5D(V1.0)-SCH.pdf
    /// DFRobot Datasheet of BMP388: https://raw.githubusercontent.com/Strictus/DFRobot/master/SEN0251/BST-BMP388-DS001-01-1307765.pdf
    #[cfg(feature = "run-pressure-and-temperature")]
    #[embassy_executor::task]
    pub async fn run_pressure_sense(i2c_mutex: &'static I2C1Mutex, mut delay: embassy_time::Delay) {
        use embassy_embedded_hal::shared_bus::I2cDeviceError;
        use protocol::SendMessage;

        info!("(bmp388) Initialise BMP388 sensor...");

        async fn log_sensor_settings(
            pressure_sensor: &mut BMP388<I2C1DeviceType, bmp388::Async>,
        ) -> Result<(), I2cDeviceError<i2c::Error>> {
            let sampling_rate = pressure_sensor.sampling_rate().await?;
            info!(
                "(bmp388) Pressure sensor sampling rate: {:?}",
                Debug2Format(&sampling_rate)
            );
            let power_control = pressure_sensor.power_control().await?;
            info!(
                "(bmp388) Pressure sensor power control: {:?}",
                Debug2Format(&power_control)
            );
            let status = pressure_sensor.status().await?;
            info!(
                "(bmp388) Pressure sensor status: {:?}",
                Debug2Format(&status)
            );
            let oversampling = pressure_sensor.oversampling().await?;
            info!(
                "(bmp388) Pressure sensor oversampling: {:?}",
                Debug2Format(&oversampling)
            );
            let filter = pressure_sensor.filter().await?;
            info!(
                "(bmp388) Pressure sensor filter: {:?}",
                Debug2Format(&filter)
            );
            let interrupt_config = pressure_sensor.interrupt_config().await?;
            info!(
                "(bmp388) Pressure sensor Interrupt config: {:?}",
                Debug2Format(&interrupt_config)
            );

            Ok(())
        }

        // DFRobot gravity BMP388
        // let address = 0x77;
        // DFRobot breakout BMP388
        let address = 0x76;
        loop {
            let mut pressure_sensor = match embassy_time::with_timeout(
                Duration::from_millis(40),
                bmp388::BMP388::new(I2cDevice::new(i2c_mutex), address, &mut delay),
            )
            .await
            {
                Ok(Ok(sensor)) => sensor,
                Ok(Err(err)) => {
                    error!("(bmp388) Failed to initialise BMP388 sensor: {:?}", err);
                    Timer::after(Duration::from_secs(2)).await;
                    continue;
                }
                Err(err) => {
                    error!("(bmp388) Bmp388::new timeout");
                    continue;
                }
            };

            info!("loop: log_sensor_settings");
            // before setting up all values
            if let Err(err) = log_sensor_settings(&mut pressure_sensor).await {
                error!("(bmp388) Failed to log sensor settings: {}", err);
                Timer::after(Duration::from_secs(2)).await;
                continue;
            }
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
            async fn force(
                sensor: &mut bmp388::BMP388<I2C1DeviceType, bmp388::Async>,
            ) -> Result<(), I2cDeviceError<i2c::Error>> {
                sensor
                    .set_power_control(bmp388::PowerControl {
                        pressure_enable: true,
                        temperature_enable: true,
                        mode: bmp388::PowerMode::Normal,
                    })
                    .await
            }
            if let Err(err) = force(&mut pressure_sensor).await {
                error!("(bmp388) Failed to set power control: {}", err);
                Timer::after(Duration::from_secs(2)).await;
                continue;
            }
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
            // log_sensor_settings(&mut pressure_sensor).await;

            info!("(bmp388) BMP388 pressure sensor initialised!");

            let mut calibrated = false;
            loop {
                let instant = embassy_time::Instant::now();

                let data = pressure_sensor.sensor_values().await;
                pub enum AltitudeMeasurement {
                    Relative,
                    SeaLevel,
                }

                let altitude = match pressure_sensor.altitude().await {
                    Ok(x) => x,
                    Err(err) => {
                        warn!(
                            "(bmp388 altitude) Failed to take altitude: '{:?}'. Try again...",
                            err
                        );
                        continue;
                    }
                };
                let altitude = match (AltitudeMeasurement::SeaLevel, calibrated) {
                    (AltitudeMeasurement::Relative, false) => {
                        info!("(bmp388) Calibrating at altitude {} meters", altitude);
                        match pressure_sensor
                            .calibrated_absolute_difference(altitude)
                            .await
                        {
                            Ok(new_sea_level) => {
                                calibrated = true;
                                info!("(bmp388) New Sea level set at: {} Pa", new_sea_level);
                            }
                            Err(err) => {
                                warn!(
                                    "(bmp388 altitude) Failed to calibrate '{:?}', try again...",
                                    err
                                );
                                continue;
                            }
                        };

                        altitude
                    }
                    (AltitudeMeasurement::Relative, true) | (AltitudeMeasurement::SeaLevel, _) => {
                        altitude
                    }
                };

                match data {
                    Ok(data) => {
                        if MEASURE_PRESSURE_EVERY > Duration::from_millis(450) {
                            info!(
                            "(bmp388 sensor_values) Pressure: {}; Temperature: {}; Altitude: {} m",
                            data.pressure, data.temperature, altitude
                        );
                        }

                        #[cfg(feature = "run-radio")]
                        if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                            use crate::application::wifi::{RADIO_SEND_CHANNEL, RECEIVER_IP};
                            use protocol::{SendMessage, SendPacket};
                            let message = SendPacket {
                                remote: (
                                    super::wifi::RECEIVER_IP.ip().octets(),
                                    super::wifi::RECEIVER_IP.port(),
                                ),
                                message: SendMessage::PressureData {
                                    sensor_data: data.clone(),
                                    altitude,
                                },
                            };

                            match RADIO_SEND_CHANNEL.try_send(message) {
                                Ok(_) => debug!("pressure sensor data sent to radio"),
                                Err(err) => {
                                    error!("Radio send channel is full. Lost message: {}", err)
                                }
                            }
                        }

                        #[cfg(feature = "run-storage")]
                        if super::storage::SD_CARD_FUNCTIONAL
                            .load(portable_atomic::Ordering::SeqCst)
                        {
                            use crate::application::storage::SD_CARD_CHANNEL;
                            use protocol::SendMessage;

                            match SD_CARD_CHANNEL.try_send(SendMessage::PressureData {
                                sensor_data: data.clone(),
                                altitude,
                            }) {
                                Ok(_) => debug!("pressure sensor data sent to sd card"),
                                Err(err) => {
                                    error!("SD card channel is full. Lost message: {}", err)
                                }
                            }
                        }

                        #[cfg(any(feature = "run-debug-uart", feature = "run-usb"))]
                        {
                            // Serial Studio format
                            #[cfg(feature = "serial-studio")]
                            let uart_msg = {
                                let mut sstudio_msg = heapless::String::<256>::new();
                                sstudio_msg
                                    .write_fmt(format_args!(
                                        "/*{},{},{}*/\n",
                                        data.temperature, altitude, data.pressure
                                    ))
                                    .unwrap();
                                sstudio
                            };
                            #[cfg(feature = "arduino-plotter")]
                            let uart_msg = {
                                let mut arduino_msg = heapless::String::<256>::new();
                                arduino_msg
                                    .write_fmt(format_args!(
                                        "Temp_c:{:.3},Alt_m:{:.3},Press_Pa:{:.3}\n",
                                        data.temperature, altitude, data.pressure
                                    ))
                                    .unwrap();
                                arduino_msg
                            };

                            #[cfg(feature = "run-debug-uart")]
                            super::debug_uart::DEBUG_UART_PIPE
                                .write_all(uart_msg.as_bytes())
                                .await;

                            #[cfg(all(
                                feature = "run-usb",
                                any(feature = "arduino-plotter", feature = "serial-studio")
                            ))]
                            // if USB is connected
                            if super::usb::USB_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                                debug!(
                                    "Send IMU data to USB pipe ({} bytes len): {:?}",
                                    uart_msg.len(),
                                    uart_msg,
                                );
                                super::usb::USB_PIPE.write_all(uart_msg.as_bytes()).await;
                            }
                        }
                    }
                    _ => {
                        // try again
                        continue;
                    }
                }

                let elapsed = instant.elapsed();
                let run_after = match MEASURE_PRESSURE_EVERY.checked_sub(elapsed) {
                    Some(run_after) => run_after,
                    None => {
                        error!(
                            "(bmp388) Reading loop took longer than the measuring time set! {}/{}",
                            elapsed, MEASURE_PRESSURE_EVERY
                        );
                        // jump straight to next iteration
                        continue;
                    }
                };
                debug!(
                    "(bmp388) Pressure and temperature reading loop took {}/{} ms",
                    elapsed, MEASURE_PRESSURE_EVERY
                );

                Timer::after(run_after).await;
            }
        }
    }
}

#[cfg(feature = "LC76G")]
pub mod gnss {
    use defmt::Debug2Format;

    // use embedded_io_async::{Read, Write};
    use embassy_rp::{
        peripherals::{PIN_7, UART1},
        uart::BufferedUart,
    };
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
    use embassy_time::{Duration, Timer};

    use crate::application::{error, info, trace, warn};

    pub use lc76g::{GnssMessage, NmeaSentence};

    // Uart rx_fifo_full_threshold
    pub const UART_READ_BUF_SIZE: usize = 126;

    pub const NMEA_SENTENCE_TERMINATOR: &str = "\r\n";

    // pub type UART_TX_PIN = 8
    // pub type UART_RX_PIN = 9
    // pub type UART_CSN_PIN =

    #[cfg(feature = "run-gnss")]
    pub static GNSS_UART_SENDER_CHANNEL: GnssUartSenderChannel = GnssUartSenderChannel::new();
    pub static GNSS_HANDLER_CHANNEL: GnssHandlerChannel = GnssHandlerChannel::new();

    #[cfg(feature = "run-gnss")]
    pub type GnssUartSenderChannel = Channel<CriticalSectionRawMutex, GnssMessage, 10>;

    #[cfg(feature = "run-gnss")]
    pub type GnssHandlerChannel =
        Channel<CriticalSectionRawMutex, heapless::Vec<nmea::ParseResult, 10>, 10>;

    #[cfg(feature = "run-gnss")]
    fn split_sentences(sentences: &str) -> Option<Lines> {
        use defmt::debug;

        let (full_sentences, partial_sentence) = sentences.rsplit_once("\r\n").unwrap();

        let full_sentences = full_sentences
            .lines()
            .map(|line| {
                debug!("NMEA Sentence ({} bytes): {}", line.bytes().len(), line);
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

    #[cfg(feature = "run-gnss")]
    #[derive(Debug)]
    pub struct Lines<'a> {
        partial_sentence: Option<&'a str>,
        parsed: heapless::Vec<Result<nmea::ParseResult, nmea::Error<'a>>, 10>,
    }

    #[cfg(feature = "run-gnss")]
    #[embassy_executor::task]
    pub async fn run_gnss(
        gnss_rst: PIN_7,
        uart: BufferedUart<'static, embassy_rp::peripherals::UART1>,
        send_channel: &'static GnssUartSenderChannel,
        gnss_handler_sender: &'static GnssHandlerChannel,
    ) {
        // let mut gnss_rst = Output::new(gnss_rst, Level::Low);

        let (mut tx, mut rx) = uart.split();

        let receive = async {
            info!("GNSS Receive: Uart reading...");
            // max message size to receive
            // leave some extra space for AT-CMD characters
            const MAX_BUFFER_SIZE: usize = 3 * UART_READ_BUF_SIZE + 16;

            // let mut rbuf: [u8; MAX_BUFFER_SIZE] = [0_u8; MAX_BUFFER_SIZE];
            let mut rbuf: [u8; 50] = [0_u8; 50];
            let mut sentences_string = heapless::String::<512>::new();
            // let mut offset: usize = 0;
            loop {
                use embedded_io_async::BufRead;
                use embedded_io_async::Read;

                let r = rx.read_exact(&mut rbuf).await;
                match r {
                    Ok(_rbuf) => {
                        match core::str::from_utf8(&rbuf) {
                            Ok(ascii_data) => {
                                // defmt::info!("GNSS receive: Read {} bytes: {}", len, ascii_data);
                                defmt::trace!(
                                    "GNSS receive: Read {} bytes: {}",
                                    rbuf.len(),
                                    ascii_data
                                );

                                // should fit the String buffer
                                sentences_string.push_str(ascii_data).unwrap();
                            }
                            Err(utf8_err) => {
                                error!(
                                    "GNSS receive: Failed to parse received GNSS bytes as utf8: {}",
                                    defmt::Debug2Format(&utf8_err)
                                );
                                warn!(
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
                                            trace!(
                                                "GNSS receive, sentence parsing: {}",
                                                defmt::Debug2Format(&err)
                                            );
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
                use embedded_io_async::Write;
                let send_gnss_sentence = send_channel.receive().await;

                let sentence_string = send_gnss_sentence.to_nmea_sentence();
                info!(
                    "Sending sentence to Gnss receiver: '{}'",
                    sentence_string.trim_end()
                );
                match tx.write_all(sentence_string.as_bytes()).await {
                    Ok(_) => info!("GNSS sentence sent!"),
                    Err(err) => error!("GNSS UART send: {:?}", err),
                };
            }
        };

        embassy_futures::select::select(receive, send).await;
    }

    /// Sets some options for the GNSS receiver
    ///
    /// Sends the message over the channel [`GnssUartSenderChannel`] to the [`run_gnss`] task.
    #[embassy_executor::task]
    pub async fn run_gnss_setup(send_channel: &'static GnssUartSenderChannel) {
        let wait_for = Duration::from_millis(50);
        info!(
            "GNSS send channel message: Wait {} milliseconds before sending...",
            wait_for.as_millis()
        );
        Timer::after(wait_for).await;
        // let baudrate = GnssMessage::SetBaudrate;
        // info!("Sending: {}", baudrate.to_nmea_sentence());
        // send_channel.send(baudrate).await;

        let gnss_providers = GnssMessage::EnableGnssProviders;
        info!("Sending: {}", gnss_providers.to_nmea_sentence());
        send_channel.send(gnss_providers).await;
    }

    #[embassy_executor::task]
    pub async fn run_gnss_handler(gnss_handler_sender: &'static GnssHandlerChannel) {
        loop {
            let sentences = gnss_handler_sender.receive().await;
            for sentence in sentences {
                match sentence {
                    nmea::ParseResult::GSA(gsa) => {
                        // info!(
                        //     "GSA - fixed sat prn ({} len): {:?}",
                        //     gsa.fix_sats_prn.len(),
                        //     gsa.fix_sats_prn
                        // );
                        #[cfg(feature = "run-radio")]
                        if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                            if let Err(err) =
                                super::wifi::RADIO_SEND_CHANNEL.try_send(protocol::SendPacket {
                                    remote: (
                                        super::wifi::RECEIVER_IP.ip().octets(),
                                        super::wifi::RECEIVER_IP.port(),
                                    ),
                                    message: protocol::SendMessage::GnssData(
                                        protocol::GnssData::GSA(gsa),
                                    ),
                                })
                            {
                                // warn!("Radio channel full, lost 1 GSA sentence: {:?}", err);
                                warn!("Radio channel full, lost 1 GSA sentence");
                            }
                        }
                    }
                    nmea::ParseResult::GSV(gsv) => {
                        // info!(
                        //     "GSV - {}, sats in view: {}",
                        //     Debug2Format(&gsv.gnss_type),
                        //     gsv.sats_in_view
                        // );
                        #[cfg(feature = "run-radio")]
                        if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                            if let Err(err) =
                                super::wifi::RADIO_SEND_CHANNEL.try_send(protocol::SendPacket {
                                    remote: (
                                        super::wifi::RECEIVER_IP.ip().octets(),
                                        super::wifi::RECEIVER_IP.port(),
                                    ),
                                    message: protocol::SendMessage::GnssData(
                                        protocol::GnssData::GSV(gsv),
                                    ),
                                })
                            {
                                // warn!("Radio channel full, lost 1 GSV sentence: {:?}", err);
                                warn!("Radio channel full, lost 1 GSV sentence");
                            }
                        }
                    }
                    nmea::ParseResult::RMC(rmc) => {
                        // info!(
                        //     "RMC - status of fix: {:?}",
                        //     Debug2Format(&rmc.status_of_fix)
                        // );
                        #[cfg(feature = "run-radio")]
                        if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                            if let Err(err) =
                                super::wifi::RADIO_SEND_CHANNEL.try_send(protocol::SendPacket {
                                    remote: (
                                        super::wifi::RECEIVER_IP.ip().octets(),
                                        super::wifi::RECEIVER_IP.port(),
                                    ),
                                    message: protocol::SendMessage::GnssData(
                                        protocol::GnssData::RMC(rmc),
                                    ),
                                })
                            {
                                // warn!("Radio channel full, lost 1 RMC sentence: {:?}", err);
                                warn!("Radio channel full, lost 1 RMC sentence");
                            }
                        }
                    }
                    nmea::ParseResult::GLL(gll) => {
                        info!(
                            "GLL - latitude: {:?}, longitude: {:?}, is valid? {}",
                            gll.latitude, gll.longitude, gll.valid
                        );
                        #[cfg(feature = "run-radio")]
                        if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                            if let Err(err) =
                                super::wifi::RADIO_SEND_CHANNEL.try_send(protocol::SendPacket {
                                    remote: (
                                        super::wifi::RECEIVER_IP.ip().octets(),
                                        super::wifi::RECEIVER_IP.port(),
                                    ),
                                    message: protocol::SendMessage::GnssData(
                                        protocol::GnssData::GLL(gll),
                                    ),
                                })
                            {
                                warn!("Radio channel full, lost 1 GLL sentence: {:?}", err);
                            }
                        }
                    }
                    _ => {
                        // skip rest of the sentences
                    }
                }
            }
        }
    }
}

#[cfg(feature = "BME688")]
mod air_quality {
    use defmt::*;

    #[cfg(feature = "run-air-quality")]
    #[embassy_executor::task]
    pub async fn run_air_quality() {
        use bme68x_rust::{
            CommInterface, Device, DeviceConfig, Error, Filter, GasHeaterConfig, Interface, Odr,
            OperationMode, Sample, SensorData,
        };

        const MEASURE_AIR_QUALITY_EVERY: Duration = Duration::from_secs(1);

        struct BME688Interface {
            i2c: std::sync::Mutex<I2CDriver<NormalMode>>,
        }

        impl Interface for BME688Interface {
            fn interface_type(&self) -> CommInterface {
                CommInterface::I2C
            }

            fn delay(&self, period: u32) {
                let delay = std::time::Duration::from_micros(period as u64);
                std::thread::sleep(delay);
            }

            fn write(&self, reg_addr: u8, reg_data: &[u8]) -> Result<(), Error> {
                let mut lock = self.i2c.lock().map_err(|lock_err| {
                    error!("I2c Lock error: {lock_err}");
                    Error::Unknown
                })?;
                lock.write(reg_addr, reg_data).map_err(|err| {
                    error!("I2c Error: {err}");
                    Error::Unknown
                })
            }

            fn read(&self, reg_addr: u8, reg_data: &mut [u8]) -> Result<(), Error> {
                let mut lock = self.i2c.lock().map_err(|lock_err| {
                    error!("I2c Lock error: {lock_err}");
                    Error::Unknown
                })?;

                lock.read(reg_addr, reg_data).map_err(|err| {
                    error!("I2c Error: {err}");
                    Error::Unknown
                })
            }
        }

        // initialize the bme68x device
        let mut bme = Device::initialize(BME688Interface {
            i2c: Mutex::new(i2c),
        })
        .unwrap();
        // .map_err(|err| anyhow!("BME688 init error: {err:?}"))?;

        // configure device
        bme.set_config(
            DeviceConfig::default()
                .filter(Filter::Off)
                .odr(Odr::StandbyNone)
                .oversample_humidity(Sample::Once)
                .oversample_pressure(Sample::X16)
                .oversample_temperature(Sample::X2),
        )
        .unwrap();
        // .map_err(|err| anyhow!("BME688 config error: {err:?}"))?;

        // configure heater
        bme.set_gas_heater_conf(
            OperationMode::Forced,
            GasHeaterConfig::default()
                .enable()
                .heater_temp(300)
                .heater_duration(100),
        )
        .unwrap();
        // .map_err(|err| anyhow!("BME688 init error: {err:?}"))?;

        let time_ms = std::time::Instant::now();
        info!("Sample, TimeStamp(ms), Temperature(deg C), Pressure(Pa), Humidity(%%), Gas resistance(ohm), Status");
        loop {
            let instant = Instant::now();

            // Set operating mode
            bme.set_op_mode(OperationMode::Forced)
                .map_err(|err| anyhow!("BME688 init error: {err:?}"))?;

            // Delay the remaining duration that can be used for heating
            let del_period = bme
                .get_measure_duration(OperationMode::Forced)
                .wrapping_add(300 as u32 * 1000);
            bme.interface.delay(del_period);

            // Get the sensor data
            let mut n_fields = 0;
            let mut data: SensorData = SensorData::default();
            bme.get_data(1, &mut data, &mut n_fields)
                .map_err(|err| anyhow!("BME688 init error: {err:?}"))?;

            if n_fields != 0 {
                info!(
                    "{:?}, {:.2}, {:.2}, {:.2} {:.2} {:x}",
                    time_ms.elapsed().as_millis(),
                    data.temperature,
                    data.pressure,
                    data.humidity,
                    data.gas_resistance,
                    data.status,
                );
            }
            let elapsed = instant.elapsed();
            tokio::time::sleep(MEASURE_AIR_QUALITY_EVERY - elapsed).await;
        }
    }
}

#[cfg(feature = "SGP40")]
mod voc_index {
    pub static SGP40_ADDRESS: u8 = 0x59;

    #[cfg(feature = "run-voc-index")]
    #[embassy_executor::task]
    pub async fn run_voc_index(i2c_mutex: &'static super::I2C0Mutex) {
        use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
        use embassy_time::{Duration, Timer};

        let i2c_device = I2cDevice::new(i2c_mutex);

        let mut sgp40 = sgp40::Sgp40::new(i2c_device, SGP40_ADDRESS, embassy_time::Delay);

        // Discard the first 45 samples as the algorithm is just warming up.
        for _ in 1..45 {
            match sgp40.measure_voc_index() {
                Ok(_warm_up) => {}
                Err(err) => defmt::error!("(shp40) Error during warm-up reading: {:?}".err),
            };

            // Reading should be done in 1 Hz intervals
            // Keep it simple - don't compensate for the reading time itself
            Timer::after(Duration::from_secs(1)).await;
        }

        loop {
            if let Ok(result) = sgp40.measure_voc_index() {
                defmt::info!("(sgp40) VOC index: {}", result);
            } else {
                defmt::error!("(sgp40) Failed I2C reading");
            }

            Timer::after(Duration::from_secs(1_u64)).await;
        }
    }
}

#[cfg(feature = "BNO055")]
mod bno055 {
    use defmt::*;

    use embassy_embedded_hal::shared_bus::blocking::i2c::I2cDevice;
    use embassy_time::Duration;

    use bno055::{mint::Quaternion, Bno055};

    #[cfg(feature = "run-flash")]
    use super::flash::FlashStorage;
    use super::I2C0Mutex;

    pub const BNO005_I2C_ADDRESS: u8 = 0x28;
    // pub const MEASURE_IMU_EVERY: Duration = Duration::from_millis(5000);
    pub const MEASURE_IMU_EVERY: Duration = Duration::from_millis(500);

    /// IMU data reading using BNO055 (**Requires calibration on start**)
    ///
    /// # Prerequisite
    ///
    /// - Requires calibration on start as described in datasheet **page 51**.
    ///
    /// Axis from datasheet (page 26):
    ///
    /// BNO055 Datasheet: <https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bno055-ds000.pdf>
    #[cfg(feature = "run-imu")]
    #[embassy_executor::task]
    pub async fn run_imu(i2c_mutex: &'static I2C0Mutex, flash_storage: &'static FlashStorage) {
        use bno055::{mint::Vector3, BNO055AxisSign};
        use embassy_time::{Duration, Timer};

        let mut delay = embassy_time::Delay;
        info!("init: bno055");
        loop {
            let i2c_blocking_dev = I2cDevice::new(i2c_mutex);

            let mut imu = Bno055::new(i2c_blocking_dev).with_alternative_address();
            if let Err(err) = imu.init(&mut delay) {
                error!("(bno055) Failed to initialise IMU, retry..");
                continue;
            }
            info!("(bno055) IMU initialized");

            loop {
                match imu.set_mode(bno055::BNO055OperationMode::NDOF, &mut delay) {
                    Ok(_) => break,
                    Err(err) => {
                        error!(
                            "(bno055) An error occurred while setting the IMU mode: {}",
                            err
                        );
                        continue;
                    }
                }
            }

            let mut status = match imu.get_calibration_status() {
                Ok(x) => x,
                Err(err) => {
                    error!(
                        "(bno055) An error occurred while getting the calibration status: {}",
                        err
                    );
                    continue;
                }
            };

            info!("(bno055) The IMU's calibration status is: {:?}", status);

            #[cfg(feature = "flash-store")]
            let store_calibration = match flash_storage.imu_calibration().await {
                Ok(Some(calibration)) => {
                    info!("(bno055) IMU calibration data found in Flash, set data on BNO055");

                    let mut store_calibration_profile = true;
                    #[cfg(not(feature = "imu-recalibrate"))]
                    if let Err(err) = imu.set_calibration_profile(calibration, &mut delay) {
                        error!(
                            "(bno055) Failed to set the Calibration Profile to BNO055: {}",
                            err
                        );
                        store_calibration_profile = false;
                    }

                    store_calibration_profile
                }
                Ok(None) => {
                    info!("(bno055) No fully-calibrated data for BNO055 found in Flash");
                    true
                }
                Err(err) => {
                    error!(
                        "(bno055) Failed to fetch calibration profile from Flash: {}",
                        err
                    );
                    true
                }
            };

            // Wait for device to auto-calibrate.
            // Please perform steps necessary for auto-calibration to kick in.
            // Required steps are described in Datasheet section 3.11
            // Page 51, https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bno055-ds000.pdf (As of 2021-07-02)
            info!("(bno055) About to begin BNO055 IMU calibration...");
            loop {
                match imu.is_fully_calibrated() {
                    Ok(false) => {
                        let mut status = match imu.get_calibration_status() {
                            Ok(x) => x,
                            Err(err) => {
                                error!(
                                    "(bno055) An error occurred while getting the calibration status: {}",
                                    err
                                );
                                continue;
                            }
                        };
                        info!(
                            "(bno055) Calibration status (1 second check period): {:?}",
                            status
                        );
                        Timer::after(Duration::from_millis(1000)).await;
                    }
                    // calibrated, break the loop
                    Ok(true) => {
                        break;
                    }
                    Err(err) => {
                        error!(
                            "(bno055) Failed to check if IMU is fully calibrated: {}",
                            err
                        );
                        Timer::after(Duration::from_millis(1000)).await;
                    }
                }
            }
            status = match imu.get_calibration_status() {
                Ok(x) => x,
                Err(err) => {
                    error!(
                        "(bno055) An error occurred while getting the calibration status: {}",
                        err
                    );
                    continue;
                }
            };
            info!("(bno055) Calibrated status: {:?}", status);

            let calib = imu.calibration_profile(&mut delay).unwrap();
            #[cfg(feature = "flash-store")]
            // No need to wear flash and write the calibration if it's already stored inside!
            if store_calibration {
                match flash_storage.write_imu_calibration(status, calib).await {
                    Ok(fully_calibrated) => {
                        if fully_calibrated {
                            info!("(bno055) Calibration profile and status written to Flash!");
                        } else {
                            info!(
                                "(bno055) Not fully calibrated, didn't write the profile to Flash!"
                            );
                        }
                    }
                    Err(err) => {
                        error!(
                            "(bno055) Failed to write calibration profile and status to Flash: {}",
                            err
                        )
                    }
                }
            }

            if let Err(err) = imu.set_calibration_profile(calib, &mut delay) {
                error!("(bno055) Failed to set calibration profile of the IMU");
            }
            info!("(bno055) Calibration complete!");

            // if let Err(err) = imu.set_axis_sign(
            //     BNO055AxisSign::X_NEGATIVE
            //         | BNO055AxisSign::Y_NEGATIVE
            //         | BNO055AxisSign::Z_NEGATIVE,
            // ) {
            //     error!("(bno055) Failed to set axis sign");
            // }

            // These are sensor fusion reading using the mint crate that the state will be read into
            let mut quaternion: Quaternion<f32>; // = Quaternion::<f32>::from([0.0, 0.0, 0.0, 0.0]);
            let mut gyro_data: Vector3<f32>;
            let mut accel_data: Vector3<f32>;
            let mut gravity: Vector3<f32>; // = Quaternion::<f32>::from([0.0, 0.0, 0.0, 0.0]);

            loop {
                let instant = embassy_time::Instant::now();
                // Quaternion; due to a bug in the BNO055, this is recommended over Euler Angles
                match imu.quaternion() {
                    Ok(val) => {
                        quaternion = val;
                    }
                    Err(e) => {
                        error!("(bno055) Quaternion: {:?}", e);
                        continue;
                    }
                }

                match imu.gyro_data() {
                    Ok(val) => {
                        gyro_data = val;
                    }
                    Err(e) => {
                        error!("(bno055) Gyro data: {:?}", e);
                        continue;
                    }
                }
                match imu.accel_data() {
                    Ok(val) => {
                        accel_data = val;
                    }
                    Err(e) => {
                        error!("(bno055) Accel data: {:?}", e);
                        continue;
                    }
                }

                match imu.gravity() {
                    Ok(val) => {
                        gravity = val;
                    }
                    Err(e) => {
                        error!("(bno055) Gravity: {:?}", e);
                        continue;
                    }
                }

                let imu_data = protocol::SendMessage::ImuData {
                    accel_data,
                    gyro_data,
                    quaternion,
                };
                info!(
                    "(bno055) {:?}; Gravity vector: {}",
                    imu_data,
                    Debug2Format(&gravity)
                );

                // 421.394862 INFO  (bno055) ImuData { accel_data: "Vector3 { x: -9.5199995, y: 0.21, z: 0.74 }", gyro_data: "Vector3 { x: 0.125, y: -0.0625, z: 0.1875 }", quaternion: "Quaternion { v: Vector3 { x: 0.48065186, y: -0.48120117, z: -0.53027344 }, s: -0.5062256 }" }; Gravity vector: Vector3 { x: -9.7699995, y: 0.22999999, z: 0.72999996 }
                // 446.396349 INFO  (bno055) ImuData { accel_data: "Vector3 { x: 0.45999998, y: 0.26999998, z: 9.78 }", gyro_data: "Vector3 { x: -0.0625, y: 0.0625, z: 0.0625 }", quaternion: "Quaternion { v: Vector3 { x: -0.02545166, y: 0.01361084, z: -0.70288086 }, s: -0.7107544 }" }; Gravity vector: Vector3 { x: 0.53, y: 0.16, z: 9.79 }

                #[cfg(feature = "run-radio")]
                if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                    let message = protocol::SendPacket {
                        remote: (
                            super::wifi::RECEIVER_IP.ip().octets(),
                            super::wifi::RECEIVER_IP.port(),
                        ),
                        message: imu_data.clone(),
                    };

                    match super::wifi::RADIO_SEND_CHANNEL.try_send(message) {
                        Ok(_) => {}
                        Err(err) => error!("Radio Channel full, missed message: {:?}", err),
                    }
                }

                #[cfg(feature = "run-storage")]
                if super::storage::SD_CARD_FUNCTIONAL.load(portable_atomic::Ordering::SeqCst) {
                    use crate::application::storage::SD_CARD_CHANNEL;
                    use protocol::SendMessage;

                    match SD_CARD_CHANNEL.try_send(imu_data) {
                        Ok(_) => debug!("pressure sensor data sent to sd card"),
                        Err(err) => {
                            error!("SD card channel is full. Lost message: {}", err)
                        }
                    }
                }

                // Serial Studio format
                #[cfg(feature = "serial-studio")]
                let uart_msg = {
                    let mut sstudio_msg = heapless::String::<256>::new();
                    sstudio_msg
                        .write_fmt(format_args!("/*{},{},{}*/\n", 23, 32, 45))
                        .unwrap();
                    // sstudio
                    todo!("proper format")
                };

                #[cfg(feature = "arduino-plotter")]
                let uart_msg = {
                    use core::fmt::Write;
                    let mut arduino_msg = heapless::String::<500>::new();
                    arduino_msg
                        .write_fmt(format_args!(
                            "ACx:{:.2},ACy:{:.2},ACz:{:.2},GYx:{:.2},GYy:{:.2},GYz:{:.2},Gx:{:.2},Gy:{:.2},Gz:{:.2},Qx:{:.2},Qy:{:.2},Qz:{:.2},Qw:{:.2}\n",
                            accel_data.x, accel_data.y, accel_data.z, gyro_data.x, gyro_data.y, gyro_data.z,
                            gravity.x, gravity.y, gravity.z, quaternion.v.x, quaternion.v.y, quaternion.v.z, quaternion.s,
                        ))
                        .unwrap();
                    arduino_msg
                };

                #[cfg(all(
                    feature = "run-usb",
                    any(feature = "arduino-plotter", feature = "serial-studio")
                ))]
                // if USB is connected
                if super::usb::USB_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                    info!(
                        "Send IMU data to USB pipe ({} bytes len): {:?}",
                        uart_msg.len(),
                        uart_msg,
                    );
                    super::usb::USB_PIPE.write_all(uart_msg.as_bytes()).await;

                    debug!(
                        "Sent data to pipe, Pipe has len: {}",
                        super::usb::USB_PIPE.len()
                    );
                }

                let elapsed = instant.elapsed();
                let run_after = match MEASURE_IMU_EVERY.checked_sub(elapsed) {
                    Some(run_after) => run_after,
                    None => {
                        error!(
                            "bno055: Reading loop took longer than the measuring time set! {}/{}",
                            elapsed, MEASURE_IMU_EVERY
                        );
                        // jump straight to next iteration
                        continue;
                    }
                };
                debug!(
                    "bno055: IMU loop took {}/{} ms",
                    elapsed.as_millis(),
                    MEASURE_IMU_EVERY.as_millis()
                );

                Timer::after(run_after).await;
            }
        }
    }
}

/// Device status
#[cfg(feature = "run-status")]
mod status {
    use defmt::{debug, error, trace};

    use embassy_futures::{join::join, select::select};
    use embassy_rp::{
        adc::{Adc, Async, Channel, Config},
        gpio::Pull,
        peripherals::{ADC_TEMP_SENSOR, PIN_24, PIN_28, PIN_29},
        Peripherals,
    };
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
    use embassy_time::{with_timeout, Duration, Instant, Timer};

    use heapless::HistoryBuffer;
    use heapless::Vec;

    use protocol::{calculate_median, Battery, Status};

    use crate::application::Irqs;

    pub const MEASURE_STATUSES_EVERY: Duration = Duration::from_secs(1);
    pub type AdcMutex = Mutex<CriticalSectionRawMutex, AdcType>;
    pub type AdcType = Adc<'static, Async>;

    // duplicates with PIO for WiFi
    // pub type VBusSensePin = PIN_28;
    pub type VSysSensePin = PIN_28;

    #[embassy_executor::task]
    pub async fn run_status(
        adc_mutex: &'static AdcMutex,
        temp_pin: ADC_TEMP_SENSOR,
        battery_pin: VSysSensePin,
    ) {
        // Fetch internal temperature - RP2040
        // TODO: 30 measurements taking the median of the values
        // TODO: HistoryBuffer of X records for 1 h, 6 h , 12 h, 24 h?!
        // for some of the longer periods we could store less accurate readings in the form of i8
        // to reduce MCU memory consumed by the buffers

        let mut measure_temperature = MeasureTemperature::new(adc_mutex, temp_pin);
        let mut measure_battery = MeasureBattery::new(adc_mutex, battery_pin);
        // a measurement each second
        // let temperature_history = HistoryBuffer::<u16, 60>::new();

        loop {
            let measure_temperature = async {
                let (raw, temp_c) = measure_temperature.measure().await;

                // temperature_history.write(raw);

                defmt::info!("Temp: {} degrees ({} raw value)", temp_c, raw);

                (raw, temp_c)
            };

            let measure_battery_percentage = async {
                let (raw, battery) = measure_battery.measure().await;

                match battery {
                    Some(battery) => {
                        defmt::info!(
                            "Battery: {}% or {}V measured ({} raw value;)",
                            battery.percentage,
                            battery.voltage,
                            raw
                        );
                    }
                    None => defmt::info!("Powered by VBUS (USB)"),
                }

                battery
            };

            let instant = Instant::now();
            match with_timeout(
                Duration::from_millis(150),
                join(measure_temperature, measure_battery_percentage),
            )
            .await
            {
                Ok(((temp_raw, temp), battery)) => {
                    let status = protocol::Status {
                        battery,
                        internal_temperature: temp,

                        #[cfg(feature = "run-usb")]
                        usb_connected: Some(
                            super::usb::USB_CONNECTED.load(portable_atomic::Ordering::SeqCst),
                        ),
                        #[cfg(not(feature = "run-usb"))]
                        usb_connected: None,
                        #[cfg(feature = "run-radio")]
                        radio_connected: Some(
                            super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst),
                        ),
                        #[cfg(not(feature = "run-radio"))]
                        radio_connected: None,
                        #[cfg(feature = "run-storage")]
                        storage_functional: Some(
                            super::storage::SD_CARD_FUNCTIONAL
                                .load(portable_atomic::Ordering::SeqCst),
                        ),
                        #[cfg(not(feature = "run-radio"))]
                        storage_functional: None,
                    };

                    defmt::info!("(device status) {:?}", status);

                    #[cfg(feature = "run-radio")]
                    if super::wifi::WIFI_CONNECTED.load(portable_atomic::Ordering::SeqCst) {
                        if let Err(err) =
                            super::wifi::RADIO_SEND_CHANNEL.try_send(protocol::SendPacket {
                                remote: (
                                    super::wifi::RECEIVER_IP.ip().octets(),
                                    super::wifi::RECEIVER_IP.port(),
                                ),
                                message: protocol::SendMessage::Status(status),
                            })
                        {
                            defmt::warn!("Radio channel full, lost 1 GSA sentence: {:?}", err);
                        }
                    }

                    #[cfg(feature = "run-debug-uart")]
                    {
                        #[cfg(feature = "arduino-plotter")]
                        let uart_msg = {
                            let mut arduino_msg = heapless::String::<256>::new();
                            arduino_msg
                                .write_fmt(format_args!(
                                    "Temp_c:{},Bat_%:{}\n",
                                    temp,
                                    battery.unwrap_or_default().percentage,
                                ))
                                .unwrap();
                            arduino_msg
                        };
                        super::debug_uart::DEBUG_UART_PIPE
                            .write_all(uart_msg.as_bytes())
                            .await
                    }
                }
                Err(timeout_err) => {} // trace!(
                                       //     "Temperature: Elapsed: {:?}; Should run in: {:?}",
                                       //     elapsed,
                                       //     run_after
                                       // );
            };

            let elapsed = instant.elapsed();
            let run_after = match MEASURE_STATUSES_EVERY.checked_sub(elapsed) {
                Some(run_after) => run_after,
                None => {
                    error!(
                        "status: Reading loop took longer than the measuring time set! {}/{}",
                        elapsed, MEASURE_STATUSES_EVERY
                    );
                    // jump straight to next iteration
                    continue;
                }
            };
            debug!(
                "status: Status loop took {}/{} ms",
                elapsed, MEASURE_STATUSES_EVERY
            );

            Timer::after(run_after).await;
        }

        // Battery Percentage - RP2040 ADC3 internal battery measurement
        // TODO: 30 measurements taking the median of the values

        // TODO: join all futures together
    }

    pub struct MeasureTemperature {
        adc: &'static AdcMutex,
        temp_sensor: Channel<'static>,
    }

    impl MeasureTemperature {
        /// The number of measurements that we do before averaging the ADC value
        ///
        /// If you change this value, make sure that `sum` cannot overflow before averaging!
        pub const MEASUREMENTS: usize = 30;
        pub const MEASURE_EVERY: Duration = Duration::from_millis(1);

        pub(crate) fn new(adc: &'static AdcMutex, temp_sensor: ADC_TEMP_SENSOR) -> Self {
            Self {
                adc,
                temp_sensor: Channel::new_temp_sensor(temp_sensor),
            }
        }

        /// Measure temperature based on internal ADC4 temperature sensor
        ///
        /// Since reading from ADC takes longer that locking itself and embassy does not
        /// switch task in that time, it's better to lock the ADC, make the measurements
        /// and then release the lock.
        pub async fn measure(&mut self) -> (u16, f32) {
            let mut sum = 0_u16;
            let mut successful_reads = 0_u16;

            let mut current_measurements: Vec<u16, { Self::MEASUREMENTS }> = Vec::new();
            {
                let mut adc_guard = self.adc.lock().await;

                for i in 1..=Self::MEASUREMENTS {
                    let instant = Instant::now();
                    match adc_guard.read(&mut self.temp_sensor).await {
                        Ok(adc_level) => {
                            // safe to push, because we use exactly Self::MEASUREMENTS loop iterations
                            current_measurements.push(adc_level).unwrap();
                            sum += adc_level;
                            successful_reads += 1;
                        }
                        Err(err) => {
                            error!("Failed to read temperature: {:?}", err)
                        }
                    }

                    let elapsed = instant.elapsed();
                    match Self::MEASURE_EVERY.checked_sub(elapsed) {
                        Some(run_after) => {
                            trace!(
                                "temperature: ADC read took: {:?} / {:?}",
                                elapsed,
                                Self::MEASURE_EVERY
                            );
                            // skip last timer
                            if i != Self::MEASUREMENTS {
                                Timer::after(run_after).await;
                            }
                        }
                        None => {
                            error!("temperature: ADC read took longer than the measuring time set! {}/{}", elapsed, Self::MEASURE_EVERY);
                            // jump straight to next iteration
                            continue;
                        }
                    };
                }
            }

            // let average = sum / successful_reads;
            // debug!("Temperature average ADC value: {}", average);
            let current_median = calculate_median(&mut current_measurements);
            let temperature = convert_to_celsius(current_median);
            debug!(
                "Temperature median: ADC value (out of {} measurements): {} ({} °C)",
                Self::MEASUREMENTS,
                current_median,
                temperature,
            );
            // debug!("Temperature: {} °C", temperature);

            // and average the result
            // let temperature = convert_to_celsius(average);

            (current_median, temperature)
        }
    }

    /// # Examples
    ///
    /// Raw value of `844` is `42.118835`
    /// Raw value of `854` is `37.43746`
    /// Raw value of `867` is `36.033066`
    /// Raw value of `880` is `25.265888`
    /// etc.
    ///
    /// ```
    /// let raw_temp = 400;
    /// ```
    /// Taken from the embassy-rp example for ADC
    /// <https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/adc.rs>
    pub fn convert_to_celsius(raw_temp: u16) -> f32 {
        // According to chapter 4.9.5. Temperature Sensor in RP2040 datasheet
        let temp = 27.0 - (raw_temp as f32 * 3.3 / 4096.0 - 0.706) / 0.001721;
        let sign = if temp < 0.0 { -1.0 } else { 1.0 };
        let rounded_temp_x10: i16 = ((temp * 10.0) + 0.5 * sign) as i16;
        (rounded_temp_x10 as f32) / 10.0
    }

    pub struct MeasureBattery {
        pin: Channel<'static>,
        adc: &'static AdcMutex,
        // last_measurements: HistoryBuffer<u16, 60>,
    }

    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct BatteryMeasurement {
        pub adc_value: u16,
        pub percentage: f32,
    }

    impl MeasureBattery {
        /// Battery charged voltage
        pub const BATTERY_MAX: f32 = 4.2;
        /// Cut-off voltage
        pub const BATTERY_MIN: f32 = 3.0;

        /// for internal 3.3v reference, add correct scale factor once ADC calibration is implemented:
        /// <https://github.com/esp-rs/esp-hal/issues/326#issuecomment-1438911773>
        pub const PRECISION_FACTOR: f32 = 3.3 / 4096.0;
        // pub const CONVERSION_FACTOR_EXTERNAL_PIN = r2_f32 / (r1_f32 + r2_f32);
        pub const VOLTAGE_DIVIDER: f32 = (470_000.0 / (470_000.0 + 470_000.0));
        pub const CONVERSION_FACTOR_EXTERNAL_PIN: f32 =
            Self::PRECISION_FACTOR / Self::VOLTAGE_DIVIDER;
        // with the internal PIN
        pub const CONVERSION_FACTOR_INTERNAL_PIN: f32 = Self::PRECISION_FACTOR * 2.0;

        /// The number of measurements that we do before averaging the ADC value
        pub const MEASUREMENTS: usize = 30;
        pub const MEASURE_EVERY: Duration = Duration::from_millis(1);

        pub fn new(adc: &'static AdcMutex, readout_pin: VSysSensePin) -> Self {
            Self {
                adc,
                pin: Channel::new_pin(readout_pin, Pull::None),
                // last_measurements: HistoryBuffer::new(),
            }
        }
        /// Measure battery voltage in per cent (%) based on internal ADC3 (GPIO29)
        /// which includes a voltage divider.
        ///
        /// Since reading from ADC takes longer that locking itself and embassy does not
        /// switch task in that time, it's better to lock the ADC, make the measurements
        /// and then release the lock.
        pub async fn measure(&mut self) -> (u16, Option<Battery>) {
            let mut current_measurements: Vec<u16, { Self::MEASUREMENTS }> = Vec::new();
            let mut sum = 0.0;
            let mut successful_reads = 0_u16;

            {
                let mut adc_guard = self.adc.lock().await;

                for i in 1..=Self::MEASUREMENTS {
                    let instant = Instant::now();
                    // read ADC level
                    // let checkpoint = embassy_time::Instant::now();
                    // let mut pin_guard = self.pin.lock().await;
                    match adc_guard.read(&mut self.pin).await {
                        Ok(adc_level) => {
                            // safe to push, because we use exactly Self::MEASUREMENTS loop iterations
                            current_measurements.push(adc_level).unwrap();
                            successful_reads += 1;
                            sum += adc_level as f32;
                        }
                        Err(_err) => {
                            error!(
                                "Failed to take an ADC battery reading on {}/{}",
                                i,
                                Self::MEASUREMENTS
                            );
                        }
                    }
                    let elapsed = instant.elapsed();
                    match Self::MEASURE_EVERY.checked_sub(elapsed) {
                        Some(run_after) => {
                            trace!(
                                "battery: ADC read took: {:?} / {:?}",
                                elapsed,
                                Self::MEASURE_EVERY
                            );
                            // skip last timer
                            if i != Self::MEASUREMENTS {
                                Timer::after(run_after).await;
                            }
                        }
                        None => {
                            error!(
                                "battery: ADC read took longer than the measuring time set! {}/{}",
                                elapsed,
                                Self::MEASURE_EVERY
                            );
                            // jump straight to next iteration
                            continue;
                        }
                    };
                }
            }

            // calculate the average value
            // let average = sum / successful_reads as f32;
            // debug!("Battery average ADC value: {}", average);
            debug!(
                "{}/{} successful reads",
                successful_reads,
                Self::MEASUREMENTS
            );

            // current median
            let current_median = calculate_median(&mut current_measurements);

            // write current_median to history
            // self.last_measurements.write(current_median);

            // ADC is 12 bit resolution
            // Resolution = 3.3V/2^12 = 3.3/4095 = 0.8mV
            // let voltage_avg = Self::to_voltage(average);
            // let percentage_avg = Self::percentage(voltage_avg);

            let voltage_median = Self::to_voltage(current_median as f32);
            let percentage_median = Self::percentage(voltage_median);

            defmt::info!(
                "Current median of {} measurements - Battery voltage: {}V ; percentage: {}%",
                Self::MEASUREMENTS,
                voltage_median,
                percentage_median
            );

            if percentage_median < 100.0 {
                (
                    current_median,
                    Some(Battery {
                        voltage: voltage_median,
                        percentage: percentage_median as u8,
                    }),
                )
            } else {
                (current_median, None)
            }
        }

        fn to_voltage(adc_value: f32) -> f32 {
            // adc_value * Self::CONVERSION_FACTOR_INTERNAL_PIN
            adc_value * Self::CONVERSION_FACTOR_EXTERNAL_PIN
        }

        // (voltage - 3.0) / (4.2 - 3.0) * 100
        fn percentage(voltage_value: f32) -> f32 {
            ((voltage_value - Self::BATTERY_MIN) / (Self::BATTERY_MAX - Self::BATTERY_MIN)) * 100.0
        }
    }
}

/// Micro SD card storage
#[cfg(feature = "run-storage")]
mod storage {
    use defmt::{error, info};
    use portable_atomic::AtomicBool;
    use serde::{Deserialize, Serialize};

    use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
    use embassy_rp::{
        gpio::{AnyPin, Level, Output},
        peripherals::PIN_5,
    };
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
    use embedded_sdmmc::{blockdevice::AsyncBlockDevice, sdcard, Block, BlockIdx};

    use protocol::{
        storage::{Error, File, Filesystem, Record, EMPTY_BLOCK},
        SendMessage,
    };

    pub type SdCardChannel =
        embassy_sync::channel::Channel<CriticalSectionRawMutex, SendMessage, 100>;
    // PIN 5 for OBC
    #[cfg(all(feature = "run-storage"))]
    pub type SdCardCSnPin = embassy_rp::peripherals::PIN_5;

    /// An atomic signalling whether or not our SD Card
    /// is functional
    pub static SD_CARD_FUNCTIONAL: AtomicBool = AtomicBool::new(false);
    pub static SD_CARD_CHANNEL: SdCardChannel = SdCardChannel::new();

    /// SD Card storage implementation
    ///
    /// This is a raw write to the SD card.
    /// It should be fully formatted and without a filesystem!
    ///
    /// We use the following schema for each block of 512 bytes:
    ///
    /// 1. 2 bytes (big-endian) - u16, CRC of the whole content of the block
    ///   (incl. msg count, see below, and excluding the CRC and empty bytes)
    /// 2. 2 bytes (big-endian) - u16, total recorded messages in block:
    ///   1 for 1 message, 2 for 2 messages, 0 for a longer than 508 bytes message (currently not utilised)!
    /// 3. Messages - individually encoded messages using [`postcard`]
    /// 4. Empty bytes are filled with value `255`
    #[cfg(feature = "run-storage")]
    #[embassy_executor::task]
    pub async fn run_storage(
        spi_mutex: &'static super::SPI0Mutex,
        sdcard_csn: SdCardCSnPin,
        receive_channel: &'static SdCardChannel,
    ) {
        use defmt::trace;
        use embassy_rp::PeripheralRef;
        use embassy_time::Timer;
        use embedded_sdmmc::SdCardError;
        use static_cell::StaticCell;

        let sdcard_csn = PeripheralRef::new(AnyPin::from(sdcard_csn));
        let sdcard_csn = Output::new(sdcard_csn, Level::High);
        let shared_csn = {
            static SHARED_CSN: StaticCell<Mutex<CriticalSectionRawMutex, Output<'static>>> =
                StaticCell::new();
            SHARED_CSN.init(Mutex::new(sdcard_csn))
        };

        // for now we have only 1 file and it's hardcoded
        let file = {
            let blocks_in_1gib = 1_073_741_824 / Block::LEN_U32;
            File {
                // starts from idx on the 1 GiB mark
                start_idx: blocks_in_1gib,
                // ends at the 11 GiB mark for a total of 10 GiB file.
                end_idx: blocks_in_1gib * 11,
            }
        };
        info!(
            "Initialize file (Start block: {}; End bLock: {})",
            file.start_idx, file.end_idx
        );

        // Supply minimum of 74 clock cycles without CS asserted.
        {
            let mut spi_guard = spi_mutex.lock().await;
            loop {
                match spi_guard.write(&[0xFF_u8; 740]).await {
                    Ok(_a) => {
                        break;
                    }
                    Err(err) => {
                        error!("Failed to write to SPI0: {}", err);
                        Timer::after_millis(500).await;
                    }
                }
            }

            info!("run-storage: 74 clock cycles without CS asserted",);
        }

        loop {
            let fut = async {
                let mut delayer = embassy_time::Delay;
                // initialize the Chip Select pin
                let mut sdcard_csn = shared_csn.lock().await;
                let spi_device = SpiDevice::new(spi_mutex, &mut *sdcard_csn);
                // Build an SD Card interface out of an SPI device, a chip-select pin and the delay object
                let e_sdcard = embedded_sdmmc::SdCard::new_async(spi_device, &mut delayer);
                // Get the card size (this also triggers card initialisation because it's not been done yet)
                info!("(storage) Initialize SD Card by fetching it's size...");
                let size = e_sdcard.num_bytes().await?;
                info!(
                    "(storage) SD Card size is {} GiB ({} bytes)",
                    size / (1024 * 1024 * 1024),
                    size
                );

                // re-configure the frequency of the bus after the SD card has been initialized
                {
                    use embassy_embedded_hal::SetConfig;
                    let mut guard = spi_mutex.lock().await;
                    // 40 mHz
                    guard.set_frequency(40_000_000);
                    info!("(storage) SD Card SPI Frequency set to 40 mHz")
                }

                let filesystem = Filesystem::new(e_sdcard);

                // 4 KiB buffer with blocks of 512
                let mut buf: [Block; 8] = core::array::from_fn(|_i| EMPTY_BLOCK.clone());

                // find EoF
                let mut current_idx = match filesystem.find_eof(file, &mut buf).await {
                    Some(eof) => eof,
                    None => {
                        return Err(Error::EoFNotFound(file));
                    }
                };

                info!(
                    "(storage) End of file found at {} for file {:?}",
                    current_idx, file
                );

                // clear the buffer after finding End-of-file
                buf.fill(EMPTY_BLOCK);

                SD_CARD_FUNCTIONAL.store(true, portable_atomic::Ordering::SeqCst);

                let mut buf_fill = [0_usize; 8];
                loop {
                    let message = receive_channel.receive().await;

                    let record = Record {
                        timestamp: embassy_time::Instant::now().as_millis(),
                        message,
                    };

                    match filesystem
                        .persist(&mut buf, &mut buf_fill, record, &mut current_idx)
                        .await
                    {
                        Ok(0) => {
                            trace!("(storage) Still filling the 4 KiB buffer");
                        }
                        Ok(written_to_sdcard) => {
                            info!(
                                "(storage) Sent {} bytes to the SD card for writing",
                                written_to_sdcard
                            )
                        }
                        // TODO: Return an error and exit loop after X tries
                        Err(err) => error!("(storage) Record to SD card failed: {}", err),
                    }
                }

                Ok::<_, Error>(())
            };

            match fut.await {
                Ok(_) => unreachable!("It's an infinite loop inside"),
                Err(err) => {
                    SD_CARD_FUNCTIONAL.store(false, portable_atomic::Ordering::SeqCst);
                    error!("SD Card Failed: {}", err);
                }
            }
        }
    }
}
