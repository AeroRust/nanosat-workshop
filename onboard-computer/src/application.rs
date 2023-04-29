// use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
#[cfg(feature = "wifi")]
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi};

use embassy_executor::Executor;
#[cfg(feature = "wifi")]
use embassy_net::{udp::UdpSocket, Config, Stack, StackResources};
use embassy_net::{IpAddress, IpListenEndpoint, Ipv4Address};
use embassy_time::{Duration, Timer};

use esp_println::println;

#[cfg(feature = "wifi")]
use esp_wifi::{
    initialize,
    wifi::{WifiController, WifiDevice, WifiEvent, WifiMode, WifiState},
};

use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Gpio8, Output, PushPull},
    peripherals::{Peripherals, UART0},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        TxRxPins,
    },
    Rtc, Uart, IO,
};

pub type OnboardLed = Gpio8<Output<PushPull>>;

pub const CONNECT_TO: IpListenEndpoint = IpListenEndpoint {
    addr: Some(IpAddress::Ipv4(Ipv4Address::UNSPECIFIED)),
    port: 8000,
};

#[cfg(feature = "wifi")]
const SSID: &str = env!("SSID");
#[cfg(feature = "wifi")]
const PASSWORD: &str = env!("PASSWORD");

// #[derive(Default)]
pub struct Application {
    uart0: Uart<'static, UART0>,
    onboard_led: OnboardLed,
    #[cfg(feature = "wifi")]
    wifi: WifiController,
    #[cfg(feature = "wifi")]
    stack: Stack<esp_wifi::wifi::WifiDevice>,
}

impl Application {
    pub fn init(peripherals: Peripherals) -> Self {
        let mut system: SystemParts = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        let mut peripheral_clock_control = system.peripheral_clock_control;
        let timer_group0 =
            TimerGroup::new(peripherals.TIMG0, &clocks, &mut peripheral_clock_control);
        let mut wdt0 = timer_group0.wdt;
        let mut timer_group1 =
            TimerGroup::new(peripherals.TIMG1, &clocks, &mut peripheral_clock_control);
        let mut wdt1 = timer_group1.wdt;

        // WIFI
        #[cfg(all(feature = "wifi", feature = "embassy-time-timg0"))]
        let (stack, wifi_controller) = {
            use nanosat::singleton;

            let config = embassy_net::Config::Dhcp(Default::default());

            let seed = 1234; // very random, very secure seed

            initialize(timer_group0.timer0, hal::Rng::new(peripherals.RNG), &clocks).unwrap();

            let (wifi_interface, controller) = esp_wifi::wifi::new(WifiMode::Sta);

            // Init network stack
            let stack = Stack::new(
                wifi_interface,
                config,
                singleton!(StackResources::<3>::new()),
                seed,
            );

            (stack, controller)
        };

        // Disable watchdog timers
        rtc.swd.disable();
        rtc.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        #[cfg(feature = "embassy-time-systick")]
        embassy::init(&clocks, system_timer);

        #[cfg(feature = "embassy-time-timg0")]
        embassy::init(&clocks, timer_group0.timer0);

        // Setup peripherals for application
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // Olimex ESP32-C3 schematics: https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf
        // Set GPIO8 as an output, and set its state high initially.
        let onboard_led = io.pins.gpio8.into_push_pull_output();

        let uart0 = {
            let config = Config {
                baudrate: 115200,
                data_bits: DataBits::DataBits8,
                parity: Parity::ParityNone,
                stop_bits: StopBits::STOP1,
            };

            let pins = TxRxPins::new_tx_rx(
                io.pins.gpio21.into_push_pull_output(),
                io.pins.gpio20.into_floating_input(),
            );

            let uart = Uart::new_with_config(
                peripherals.UART0,
                Some(config),
                Some(pins),
                &clocks,
                &mut peripheral_clock_control,
            );

            // Is this for waiting the UART initialisation?
            let wait_for_init = 250u64.millis();
            println!(
                "Wait for uart to be set up with the timer of {} milliseconds..",
                wait_for_init.to_millis()
            );
            timer_group1.timer0.start(wait_for_init);
            uart
        };
        Self {
            onboard_led,
            uart0,
            #[cfg(feature = "wifi")]
            wifi: wifi_controller,
            #[cfg(feature = "wifi")]
            stack,
        }
    }

    pub fn run(self, executor: &'static mut Executor) -> ! {
        executor.run(|spawner| {
            #[cfg(feature = "wifi")]
            spawner.must_spawn(wifi(self.wifi));
            #[cfg(feature = "wifi")]
            spawner.must_spawn(net_task(&stack));
            spawner.must_spawn(blink(self.onboard_led));
        })
    }
}

#[embassy_executor::task]
async fn bing_print() {
    loop {
        esp_println::println!("Bing!");
        Timer::after(Duration::from_millis(5_000)).await;
    }
}

#[embassy_executor::task]
async fn blink(mut led: OnboardLed) {
    loop {
        led.set_high().expect("Should set High");
        // esp_println::println!("async LED ON");
        Timer::after(Duration::from_millis(100)).await;

        led.set_low().expect("Should set Low");
        // esp_println::println!("async LED OFF");
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
#[cfg(feature = "wifi")]
async fn wifi(mut controller: WifiController) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: PASSWORD.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
#[cfg(feature = "wifi")]
async fn net_task(stack: &'static Stack<WifiDevice>) {
    stack.run().await
}
