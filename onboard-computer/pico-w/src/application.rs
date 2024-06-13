use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::{
    asynch::i2c::I2cDevice, blocking::i2c::I2cDevice as BlockingI2cDevice,
};

// #[cfg(feature = "rp2040")]
use embassy_executor::{Executor, Spawner};
use embassy_futures::{join::join, select};
use embassy_rp::{
    bind_interrupts,
    gpio::{Level, Output},
    i2c::{self, I2c, InterruptHandler},
    multicore::{spawn_core1, Stack as MulticoreStack},
    peripherals::{CORE1, DMA_CH0, I2C0, I2C1, PIN_23, PIN_25, PIN_4, PIN_7, PIO0, UART0, UART1},
    pio::{self, Pio},
    uart::{
        self, BufferedInterruptHandler, BufferedUart, BufferedUartRx, Config, DataBits, Parity,
        StopBits,
    },
};
#[cfg(feature = "rp2040")]
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

use static_cell::{make_static, StaticCell};

use defmt::{error, info, trace, unwrap, warn};

#[cfg(feature = "rp2040")]
bind_interrupts!(struct Irqs {
    UART1_IRQ => BufferedInterruptHandler<UART1>;
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    // UART1_IRQ => BufferedInterruptHandler<UART1>;
});

pub type I2C0DeviceType =
    I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, I2C0, i2c::Async>>;
pub type I2C0Mutex = Mutex<CriticalSectionRawMutex, I2c<'static, I2C0, i2c::Async>>;

pub type I2C1DeviceType =
    BlockingI2cDevice<'static, CriticalSectionRawMutex, RefCell<I2c<'static, I2C1, i2c::Async>>>;
pub type I2C1Mutex =
    BlockingMutex<CriticalSectionRawMutex, RefCell<I2c<'static, I2C1, i2c::Async>>>;

pub type DebugUartPipe = Pipe<NoopRawMutex, 1024>;

#[cfg(feature = "rp2040")]
/// Stack - Core1 stack = Core 0 stack size.
static CORE0_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(feature = "rp2040")]
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
#[cfg(feature = "rp2040")]
// TODO: Set a stack size for the second core
static mut CORE1_STACK: MulticoreStack<{ 50 * 1024 }> = MulticoreStack::new();

#[embassy_executor::task]
pub async fn run_debug_uart(bytes_pipe: &'static DebugUartPipe) {
    loop {
        let mut buf = [0; 256];
        let bytes_read = bytes_pipe.read(&mut buf).await;
        let bytes_to_send = &buf[..bytes_read];
    }
}

#[cfg(feature = "cyw43")]
mod wifi {
    use static_cell::make_static;

    use embassy_net::{Config, IpEndpoint, Stack, StackResources};
    use embassy_rp::{
        gpio::Output,
        peripherals::{DMA_CH0, PIN_23, PIN_25, PIN_29, PIO0},
    };
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

    use cyw43_pio::PioSpi;

    use protocol::{ReceiveMessage, SendMessage, SendPacket};

    pub type PIN_PWR = PIN_23;
    pub type PIN_CS = PIN_25;

    pub type Cyw43_SPI<'a> = PioSpi<'a, PIO0, 0, DMA_CH0>;
    pub type RadioSendChannel =
        embassy_sync::channel::Channel<CriticalSectionRawMutex, SendPacket, 100>;
    pub type RadioReceiveChannel =
        embassy_sync::channel::Channel<CriticalSectionRawMutex, ReceiveMessage, 100>;

    pub use shared::*;

    /// TODO: make application or build configurable!
    mod shared {
        use embassy_net::IpEndpoint;

        use postcard::experimental::schema::Schema;
        use serde::{Deserialize, Serialize};
        // pub const RECEIVER_IP: ([u8; 4], u16) = ([192, 168, 88, 61], 8003);
        // WIFI_REMOTE_DESTINATION_IP
        // WIFI_REMOTE_DESTINATION_PORT
        pub const RECEIVER_IP: ([u8; 4], u16) = ([192, 168, 0, 215], 8003);
    }

    /// The WiFI network SSID to use for the WiFi `run-radio` feature
    /// Set in the `build.rs`` using a `.env` file in `onboard-computer`
    #[cfg(feature = "run-radio")]
    const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
    /// The WiFI network password to use for the WiFi `run-radio` feature
    /// Set in the `build.rs`` using a `.env` file in `onboard-computer`
    #[cfg(feature = "run-radio")]
    const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

    #[cfg(feature = "run-radio")]
    #[embassy_executor::task]
    pub async fn run_radio(
        state: &'static mut cyw43::State,
        pwr: PIN_PWR,
        spi: Cyw43_SPI<'static>,
        receive_channel: &'static RadioReceiveChannel,
        send_channel: &'static RadioSendChannel,
    ) {
        use defmt::{error, info, unwrap, warn};
        use embassy_net::{
            udp::{PacketMetadata, UdpSocket},
            IpEndpoint, Ipv4Address,
        };
        use embassy_rp::gpio::{Level, Output};
        use embassy_time::{Duration, Timer};
        use embedded_io_async::Write as _;

        let pwr = Output::new(pwr, Level::Low);

        info!("Setting up Radio - WiFi...");

        let fw = include_bytes!("../../../cyw43-firmware/43439A0.bin");
        let clm = include_bytes!("../../../cyw43-firmware/43439A0_clm.bin");

        let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

        let spawner = embassy_executor::Spawner::for_current_executor().await;

        spawner.must_spawn(wifi_task(runner));
        info!("WiFi task spawned");

        let stack = {
            control.init(clm).await;
            control
                .set_power_management(cyw43::PowerManagementMode::PowerSave)
                .await;
            let config = Config::dhcpv4(Default::default());
            // Generate random seed
            let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guaranteed to be random.

            &*make_static!(embassy_net::Stack::new(
                net_device,
                config,
                make_static!(StackResources::<2>::new()),
                seed,
            ))
        };

        spawner.must_spawn(net_task(stack));
        info!("Net task spawned");

        // And now we can use it!
        let mut rx_buffer = make_static!([0; 4096]);
        let mut tx_buffer = make_static!([0; 4096]);
        let rx_meta = make_static!([PacketMetadata::EMPTY; 16]);
        let tx_meta = make_static!([PacketMetadata::EMPTY; 16]);
        let mut socket = UdpSocket::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);
        unwrap!(socket.bind(1234));

        let socket: &'static UdpSocket<'static> = &*make_static!(socket);

        // To make flashing faster for development, you may want to flash the firmwares independently
        // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
        //     probe-rs download 43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
        //     probe-rs download 43439A0_clm.bin --format bin --chip RP2040 --base-address 0x10140000
        //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
        //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };
        loop {
            loop {
                match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
                    Ok(_) => {
                        info!("Joined WiFi with SSID: {}", WIFI_NETWORK);

                        // set status LED to high for "connected to WiFi" status
                        control.gpio_set(0, true).await;
                        break;
                    }
                    Err(err) => {
                        info!("join failed with status={}", err.status);
                    }
                }
            }

            // Wait for DHCP, not necessary when using static IP
            info!("waiting for DHCP...");
            while !stack.is_config_up() {
                Timer::after_millis(100).await;
            }
            info!("DHCP is now up!");
            info!("{:#?}", defmt::Debug2Format(&stack.config_v4()));

            let send = async {
                let mut buf = [0; 4096];
                loop {
                    let send_packet = send_channel.receive().await;

                    let slice = match send_packet.message.to_radio(&mut buf) {
                        Ok(x) => x,
                        Err(err) => {
                            error!("Failed to serialise radio packet: {}", send_packet);
                            continue;
                        }
                    };

                    let ip_endpoint =
                        IpEndpoint::from((Ipv4Address(send_packet.remote.0), send_packet.remote.1));
                    match socket.send_to(slice, ip_endpoint).await {
                        Ok(()) => {
                            defmt::debug!("packet sent to {}", defmt::Debug2Format(&ip_endpoint));
                        }
                        Err(e) => {
                            warn!("write error: {:?}", e);
                            break;
                        }
                    };
                }
            };

            let receive = async {
                let mut buf = [0; 4096];

                loop {
                    let (n, remote) = match socket.recv_from(&mut buf).await {
                        Ok(x) => x,
                        Err(e) => {
                            warn!("read error: {:?}", e);
                            break;
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
    async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
        stack.run().await
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

    use super::{wifi::RadioSendChannel, DebugUartPipe, I2C0DeviceType, I2C0Mutex};

    // I2C1
    // pub type I2C_SDA_PIN= PIN_4
    // scl 5

    const MEASURE_PRESSURE_EVERY: Duration = Duration::from_millis(600);

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
    pub async fn run_pressure_sense(
        i2c_mutex: &'static I2C0Mutex,
        mut delay: embassy_time::Delay,
        uart_pipe: &'static DebugUartPipe,
        send_channel: &'static RadioSendChannel,
    ) {
        use protocol::SendMessage;

        info!("Initialise BMP388 sensor...");

        async fn log_sensor_settings(pressure_sensor: &mut BMP388<I2C0DeviceType, bmp388::Async>) {
            let sampling_rate = pressure_sensor.sampling_rate().await.unwrap();
            info!(
                "Pressure sensor sampling rate: {:?}",
                Debug2Format(&sampling_rate)
            );
            let power_control = pressure_sensor.power_control().await.unwrap();
            info!(
                "Pressure sensor power control: {:?}",
                Debug2Format(&power_control)
            );
            let status = pressure_sensor.status().await.unwrap();
            info!("Pressure sensor status: {:?}", Debug2Format(&status));
            let oversampling = pressure_sensor.oversampling().await.unwrap();
            info!(
                "Pressure sensor oversampling: {:?}",
                Debug2Format(&oversampling)
            );
            let filter = pressure_sensor.filter().await.unwrap();
            info!("Pressure sensor filter: {:?}", Debug2Format(&filter));
            let interrupt_config = pressure_sensor.interrupt_config().await.unwrap();
            info!(
                "Pressure sensor Interrupt config: {:?}",
                Debug2Format(&interrupt_config)
            );
        }

        let address = 0x77;
        loop {
            let mut pressure_sensor =
                match bmp388::BMP388::new(I2cDevice::new(i2c_mutex), address, &mut delay).await {
                    Ok(sensor) => sensor,
                    Err(err) => {
                        error!("Failed to initialise BMP388 sensor: {:?}", err);
                        Timer::after(Duration::from_secs(2)).await;
                        continue;
                    }
                };

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
            // log_sensor_settings(&mut pressure_sensor).await;

            info!("BMP388 pressure sensor initialised!");

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
                        info!("BMP388 Calibrating at altitude {} meters", altitude);
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
                        info!("New Sea level set at: {} Pa", new_sea_level);

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

                        #[cfg(feature = "run-radio")]
                        {
                            use crate::application::wifi::RECEIVER_IP;
                            use protocol::{SendMessage, SendPacket};
                            let message = SendPacket {
                                remote: RECEIVER_IP,
                                message: SendMessage::PressureData {
                                    sensor_data: data.clone(),
                                    altitude,
                                },
                            };

                            match send_channel.try_send(message) {
                                Ok(_) => debug!("pressure sensor data sent to radio"),
                                Err(err) => {
                                    error!("Radio send channel is full. Lost message: {}", err)
                                }
                            }
                            // if let Err(err) = send_channel.try_send(message) {
                            //     error!("Radio send channel is full. Lost message: {}", err)
                            // }
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
}

pub struct Application {
    core1: CORE1,
    #[cfg(any(feature = "run-pressure-and-temperature"))]
    i2c0: I2c<'static, I2C0, i2c::Async>,
    #[cfg(any(feature = "run-imu"))]
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
}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init(/* peripherals: Peripherals */) -> Self {
        let peripherals = embassy_rp::init(Default::default());

        #[cfg(feature = "run-pressure-and-temperature")]
        let i2c0 = I2c::new_async(
            peripherals.I2C0,
            peripherals.PIN_5,
            peripherals.PIN_4,
            Irqs,
            {
                let mut config = i2c::Config::default();
                // 400 KHz
                config.frequency = 400_000;
                config
            },
        );

        #[cfg(feature = "run-imu")]
        let i2c1 = I2c::new_async(
            peripherals.I2C1,
            peripherals.PIN_3,
            peripherals.PIN_2,
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

            let tx_buf = &mut make_static!([0u8; 1024])[..];
            let rx_buf = &mut make_static!([0u8; 1024])[..];
            let gnss_uart = BufferedUart::new(
                uart1,
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

        info!("Peripherals initialised");

        Self {
            core1: peripherals.CORE1,
            #[cfg(any(feature = "run-pressure-and-temperature"))]
            i2c0,
            #[cfg(any(feature = "run-imu"))]
            i2c1,
            #[cfg(feature = "run-gnss")]
            gnss_rst,
            #[cfg(feature = "run-gnss")]
            uart1: gnss_uart,
            #[cfg(feature = "run-radio")]
            radio: (peripherals.PIN_23, cyw43_spi),
        }
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self /* , executor: &'static mut Executor */) -> ! {
        #[cfg(feature = "run-radio")]
        let (state, pwr, spi) = {
            let pwr = self.radio.0;

            let state = make_static!(cyw43::State::new());

            (state, pwr, self.radio.1)
        };

        #[cfg(feature = "run-radio")]
        let (radio_receive, radio_send) = (
            &*make_static!(self::wifi::RadioReceiveChannel::new()),
            &*make_static!(self::wifi::RadioSendChannel::new()),
        );

        /// Blocking APIs
        spawn_core1(self.core1, unsafe { &mut CORE1_STACK }, move || {
            let core1_executor = CORE1_EXECUTOR.init(Executor::new());

            core1_executor.run(|spawner| {
                #[cfg(feature = "dummy-print")]
                spawner.must_spawn(print());

                #[cfg(any(feature = "run-imu"))]
                let i2c1_mutex = &*make_static!(BlockingMutex::<CriticalSectionRawMutex, _>::new(
                    RefCell::new(self.i2c1)
                ));

                #[cfg(any(feature = "run-imu"))]
                {
                    spawner.must_spawn(bno055::run_imu(i2c1_mutex, radio_send))
                }
            })
        });
        let core0_executor = CORE0_EXECUTOR.init(Executor::new());
        core0_executor.run(|spawner| {
            let uart_pipe = make_static!(DebugUartPipe::new());
            #[cfg(any(feature = "run-pressure-and-temperature"))]
            let i2c0_mutex = make_static!(Mutex::<CriticalSectionRawMutex, _>::new(self.i2c0));
            // spawner.must_spawn(blinky(led));
            spawner.must_spawn(run_debug_uart(uart_pipe));

            #[cfg(feature = "run-radio")]
            {
                // spawner.must_spawn(wifi::run_radio(  net_device, control, runner));
                spawner.must_spawn(wifi::run_radio(state, pwr, spi, radio_receive, radio_send));
            }

            #[cfg(feature = "run-pressure-and-temperature")]
            {
                spawner.must_spawn(bmp388::run_pressure_sense(
                    i2c0_mutex, Delay, uart_pipe, radio_send,
                ));
            }
            #[cfg(feature = "run-gnss")]
            {
                let gnss_send_channel = make_static!(gnss::GnssUartSenderChannel::new());

                let gnss_handler_channel = make_static!(gnss::GnssHandlerChannel::new());

                spawner.must_spawn(gnss::run_gnss(
                    self.gnss_rst,
                    self.uart1,
                    gnss_send_channel,
                    gnss_handler_channel,
                ));
                spawner.must_spawn(gnss::run_gnss_setup(gnss_send_channel));
                spawner.must_spawn(gnss::run_gnss_handler(gnss_handler_channel, radio_send));
            }
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
async fn blinky(mut led: Output<'static>) {
    loop {
        info!("led on!");
        led.set_high();
        Timer::after(Duration::from_secs(1)).await;

        info!("led off!");
        led.set_low();
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[cfg(feature = "LC76G")]
pub mod gnss {
    use defmt::Debug2Format;
    use embassy_rp::{
        peripherals::{PIN_7, UART1},
        uart::BufferedUart,
    };
    use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
    use embassy_time::{Duration, Timer};

    use crate::application::{error, info, trace, warn};

    pub use lc76g::{GnssMessage, NmeaSentence};

    use super::wifi::RECEIVER_IP;

    // Uart rx_fifo_full_threshold
    const UART_READ_BUF_SIZE: usize = 126;

    pub const NMEA_SENTENCE_TERMINATOR: &str = "\r\n";

    // pub type UART_TX_PIN = 8
    // pub type UART_RX_PIN = 9
    // pub type UART_CSN_PIN =

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

        let (mut rx, mut tx) = uart.split();

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

                // let r = rx.read(&mut rbuf).await;
                // let r = rx.fill_buf().await;
                let r = rx.read_exact(&mut rbuf).await;
                // let r_len = r.map(|buf| buf.len()).unwrap_or_default();
                match r {
                    Ok(_rbuf) => {
                        // Ok(len) => {
                        // match core::str::from_utf8(&rbuf[..len]) {
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

                // rx.consume(r_len);

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
        // use embedded_io_async::BufRead;

        // let (mut rx, mut tx) = uart.split();

        // // Timer::after(Duration::from_secs(1)).await;
        // // gnss_rst.set_low();
        // // Timer::after(Duration::from_secs(1)).await;

        // const BUF_SIZE: usize = 256;
        // let mut buf_string = heapless::String::<{ BUF_SIZE }>::new();

        // loop {
        //     let read_len = {
        //         info!("reading...");
        //         let read_buf = match rx.fill_buf().await {
        //             Ok(x) => x,
        //             Err(err) => {
        //                 error!("UART: {:?}", defmt::Debug2Format(&err));
        //                 warn!("Clear remaining buffer: '{}'", buf_string);
        //                 buf_string.clear();
        //                 continue;
        //             }
        //         };

        //         info!(
        //             "Read {} more bytes, string length so far: {}",
        //             read_buf.len(),
        //             buf_string.len()
        //         );
        //         let read_str = core::str::from_utf8(read_buf).unwrap();
        //         unwrap!(buf_string.push_str(read_str));
        //         info!("String messages:\n'{}'", buf_string);

        //         read_buf.len()
        //     };
        //     rx.consume(read_len);

        //     if buf_string.contains("\r\n") {
        //         let (partial_sentence, sentences) = match split_sentences(buf_string.as_str()) {
        //             Some(lines) => {
        //                 let partial_sentence = lines
        //                     .partial_sentence
        //                     .map(|string| unwrap!(heapless::String::<50>::try_from(string)));
        //                 let sentences = lines
        //                     .parsed
        //                     .into_iter()
        //                     .filter_map(|result| match result {
        //                         Ok(sentence) => Some(sentence),
        //                         Err(err) => {
        //                             error!("{}", defmt::Debug2Format(&err));
        //                             None
        //                         }
        //                     })
        //                     .collect::<heapless::Vec<nmea::ParseResult, 10>>();

        //                 (partial_sentence, sentences)
        //             }
        //             None => {
        //                 continue;
        //             }
        //         };

        //         info!(
        //             "Full sentences parsed: {:?}",
        //             defmt::Debug2Format(&sentences)
        //         );
        //         info!(
        //             "partial: {}",
        //             partial_sentence.clone().unwrap_or_default().as_str()
        //         );
        //         buf_string.clear();
        //         if let Some(partial_sentence) = partial_sentence {
        //             buf_string.push_str(partial_sentence.as_str()).unwrap();
        //         }
        //     }
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
    pub async fn run_gnss_handler(
        gnss_handler_sender: &'static GnssHandlerChannel,
        send_channel: &'static crate::application::wifi::RadioSendChannel,
    ) {
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
                        if let Err(err) = send_channel.try_send(protocol::SendPacket { remote: RECEIVER_IP, message: protocol::SendMessage::GnssData(
                            protocol::GnssData::GSA(gsa),
                        )}) {
                            warn!("Radio channel full, lost 1 GSA sentence: {:?}", err);
                        }
                        
                    }
                    nmea::ParseResult::GSV(gsv) => {
                        // info!(
                        //     "GSV - {}, sats in view: {}",
                        //     Debug2Format(&gsv.gnss_type),
                        //     gsv.sats_in_view
                        // );
                        #[cfg(feature = "run-radio")]
                        if let Err(err) = send_channel.try_send(protocol::SendPacket { remote: RECEIVER_IP, message: protocol::SendMessage::GnssData(
                            protocol::GnssData::GSV(gsv),
                        )}) {
                            warn!("Radio channel full, lost 1 GSV sentence: {:?}", err);
                        }
                    }
                    nmea::ParseResult::RMC(rmc) => {
                        // info!(
                        //     "RMC - status of fix: {:?}",
                        //     Debug2Format(&rmc.status_of_fix)
                        // );
                        #[cfg(feature = "run-radio")]
                        if let Err(err) = send_channel.try_send(protocol::SendPacket { remote: RECEIVER_IP, message: protocol::SendMessage::GnssData(
                            protocol::GnssData::RMC(rmc),
                        )}) {
                            warn!("Radio channel full, lost 1 RMC sentence: {:?}", err);
                        }
                    }
                    nmea::ParseResult::GLL(gll) => {
                        info!(
                            "GLL - latitude: {:?}, longitude: {:?}, is valid? {}",
                            gll.latitude, gll.longitude, gll.valid
                        );
                        #[cfg(feature = "run-radio")]
                        if let Err(err) = send_channel.try_send(protocol::SendPacket { remote: RECEIVER_IP, message: protocol::SendMessage::GnssData(
                            protocol::GnssData::GLL(gll),
                        )}) {
                            warn!("Radio channel full, lost 1 GLL sentence: {:?}", err);
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

    use bno055::{
        mint::{EulerAngles, Quaternion},
        Bno055,
    };

    use super::I2C1Mutex;

    pub const BNO005_I2C_ADDRESS: u8 = 0x28;

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
    pub async fn run_imu(
        i2c_mutex: &'static I2C1Mutex,
        send_channel: &'static super::wifi::RadioSendChannel,
    ) {
        use bno055::{mint::Vector3, BNO055AxisSign};
        use embassy_time::{Duration, Timer};

        let mut delay = embassy_time::Delay;
        loop {
            let i2c_blocking_dev = I2cDevice::new(i2c_mutex);

            let mut imu = Bno055::new(i2c_blocking_dev).with_alternative_address();
            imu.init(&mut delay).unwrap();

            imu.set_mode(bno055::BNO055OperationMode::NDOF, &mut delay)
                .expect("An error occurred while setting the IMU mode");

            let mut status = imu.get_calibration_status().unwrap();
            info!(
                "The IMU's calibration status is: {:?}",
                Debug2Format(&status)
            );

            // Wait for device to auto-calibrate.
            // Please perform steps necessary for auto-calibration to kick in.
            // Required steps are described in Datasheet section 3.11
            // Page 51, https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bno055-ds000.pdf (As of 2021-07-02)
            info!("- About to begin BNO055 IMU calibration...");
            while !imu.is_fully_calibrated().unwrap() {
                status = imu.get_calibration_status().unwrap();
                Timer::after(Duration::from_millis(1000)).await;
                info!("Calibration status: {:?}", Debug2Format(&status));
            }

            let calib = imu.calibration_profile(&mut delay).unwrap();

            imu.set_calibration_profile(calib, &mut delay).unwrap();
            info!("       - Calibration complete!");

            imu.set_axis_sign(
                BNO055AxisSign::X_NEGATIVE
                    | BNO055AxisSign::Y_NEGATIVE
                    | BNO055AxisSign::Z_NEGATIVE,
            )
            .unwrap();

            // These are sensor fusion reading using the mint crate that the state will be read into
            // let mut euler_angles: EulerAngles<f32, ()>; // = EulerAngles::<f32, ()>::from([0.0, 0.0, 0.0]);
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
                        error!("Quaternion: {:?}", Debug2Format(&e));
                        continue;
                    }
                }

                match imu.gyro_data() {
                    Ok(val) => {
                        gyro_data = val;
                    }
                    Err(e) => {
                        error!("Gyro data: {:?}", Debug2Format(&e));
                        continue;
                    }
                }
                match imu.accel_data() {
                    Ok(val) => {
                        accel_data = val;
                    }
                    Err(e) => {
                        error!("Accel data: {:?}", Debug2Format(&e));
                        continue;
                    }
                }

                match imu.gravity() {
                    Ok(val) => {
                        gravity = val;
                    }
                    Err(e) => {
                        error!("Gravity: {:?}", Debug2Format(&e));
                        continue;
                    }
                }

                let message = protocol::SendPacket {
                    remote: crate::application::wifi::RECEIVER_IP,
                    message: protocol::SendMessage::ImuData {
                        accel_data,
                        gyro_data,
                        quaternion,
                    },
                };

                info!(
                    "(bno055) {:?}; Gravity vector: {}",
                    message,
                    Debug2Format(&gravity)
                );

                match send_channel.try_send(message) {
                    Ok(_) => {}
                    Err(err) => error!("Radio Channel full, missed message: {:?}", err),
                }
                Timer::after(Duration::from_millis(500) - instant.elapsed()).await;
            }
        }
    }
}
